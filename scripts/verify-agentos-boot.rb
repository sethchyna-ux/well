#!/usr/bin/env ruby
# A real firmware-to-AgentOS regression; no mocked shell output.
require 'open3'
require 'timeout'
require 'digest'
companion, output, mode, network_url = ARGV
abort 'Usage: verify-agentos-boot.rb /path/to/well-boot /path/to/artifacts [network]' unless companion && output
args = [companion, 'run', '--output', output]
args << '--network' if mode == 'network'
args += ['--network-url',network_url] if network_url
expected = Digest::SHA256.hexdigest('WellBootVerified')
log_path = File.join(output, network_url ? (network_url.start_with?('http:') ? 'qemu-companion-http.log' : 'qemu-companion-tftp.log') : (mode == 'network' ? 'qemu-network.log' : 'qemu-disk.log'))
passed = false
Open3.popen2e(*args, pgroup: true) do |input, stream, worker|
  transcript = +''
  sent = false
  begin
    Timeout.timeout(90) do
      File.open(log_path, 'wb') do |log|
        loop do
          chunk = stream.readpartial(8192)
          log.write(chunk); log.flush
          transcript << chunk
          if !sent && transcript.include?('3-PANE GRAPHICAL SHELL') && transcript.end_with?('trismegistus# ')
            input.write("sha256 WellBootVerified\r"); input.flush; sent = true
          end
          if sent && transcript.include?(expected)
            raise 'Missing validated boot handoff' unless transcript.include?('Protocol Magic Verified')
            passed = true
            break
          end
        end
      end
    end
  rescue EOFError, Timeout::Error => e
    warn "Firmware verification failed: #{e.class}; inspect #{log_path}"
  ensure
    begin
      Process.kill('TERM', -worker.pid)
    rescue Errno::ESRCH
    end
    worker.value
  end
end
abort "FAIL: #{log_path}" unless passed
puts "PASS: AgentOS firmware handoff and SHA-256 applet response (#{log_path})"
