#!/usr/bin/env ruby
# Real loopback HTTP and TFTP transfer against the netboot companion.
require 'socket'
require 'timeout'
require 'digest'
require 'json'
companion, output = ARGV
abort 'Usage: verify-agentos-services.rb /path/to/well-boot /path/to/artifacts' unless companion && output
ports = 4.times.map do
  s=UDPSocket.new; s.bind('127.0.0.1',0); p=s.addr[1]; s.close; p
end
tcp=TCPServer.new('127.0.0.1',0); http=tcp.addr[1]; tcp.close
pid=Process.spawn(companion,'serve','--output',output,'--bind-ip','127.0.0.1','--dhcp-port',ports[0].to_s,'--proxy-port',ports[1].to_s,'--tftp-port',ports[2].to_s,'--http-port',http.to_s,pgroup:true,out:File.join(output,'service-test.log'),err:[:child,:out])
def get(port,path)
  socket=TCPSocket.new('127.0.0.1',port)
  socket.write("GET #{path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
  response=socket.read;socket.close;response
end
begin
  Timeout.timeout(10) do
    loop do
      begin
        break if get(http,'/status').start_with?('HTTP/1.1 200')
      rescue Errno::ECONNREFUSED
      end
      sleep 0.05
    end
  end
  raise 'Unexpected HTTP route' unless get(http,'/missing').start_with?('HTTP/1.1 404')
  raise 'HTTP payload unavailable' unless get(http,'/BOOTX64.EFI').include?("\r\n\r\nMZ")
  status = JSON.parse(get(http,'/status').split("\r\n\r\n",2).last)
  raise 'HTTP requests not recorded' unless status.fetch('metrics').fetch('http_requests') >= 3
  socket=UDPSocket.new;socket.bind('127.0.0.1',0)
  payload=+''.b
  socket.send([1].pack('n')+"ipxe.efi\0octet\0",0,'127.0.0.1',ports[2])
  block=1
  Timeout.timeout(30) do
    loop do
      packet,peer=socket.recvfrom(2048)
      opcode,number=packet.unpack('nn')
      raise "Unexpected TFTP opcode/block #{opcode}/#{number}" unless opcode==3 && number==block
      bytes=packet.byteslice(4..);payload<<bytes
      socket.send([4,block].pack('nn'),0,peer[3],peer[1])
      break if bytes.bytesize<512
      block=(block+1)&0xffff
    end
  end
  socket.close
  raise 'TFTP checksum mismatch' unless Digest::SHA256.hexdigest(payload)==Digest::SHA256.file(File.join(output,'BOOTX64.EFI')).hexdigest
  puts 'PASS: real HTTP routes and complete TFTP EFI transfer matched exported SHA-256'
  ['http','tftp'].each do |protocol|
    port=protocol=='http' ? http : ports[2]
    script=File.join(__dir__,'verify-agentos-boot.rb')
    raise "Firmware #{protocol} boot failed" unless system('ruby',script,companion,output,'network',"#{protocol}://10.0.2.2:#{port}/BOOTX64.EFI")
  end

ensure
  Process.kill('INT',-pid) rescue Errno::ESRCH
  Process.wait(pid)
end
