// cli_page.dart – UI for invoking the bundled well‑cli binary on Android
import 'dart:async';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:path_provider/path_provider.dart';

class CliPage extends StatefulWidget {
  const CliPage({Key? key}) : super(key: key);

  @override
  State<CliPage> createState() => _CliPageState();
}

class _CliPageState extends State<CliPage> {
  String _output = '';
  bool _busy = false;
  String? _abi;

  @override
  void initState() {
    super.initState();
    _detectAbi();
  }

  // Use a MethodChannel to ask Android for the primary ABI.
  static const _channel = MethodChannel('well/abi');

  Future<void> _detectAbi() async {
    String abi = 'arm64-v8a'; // fallback
    try {
      final String result = await _channel.invokeMethod<String>('getAbi') ?? abi;
      abi = result;
    } catch (_) {}
    setState(() => _abi = abi);
  }

  Future<File> _prepareExecutable() async {
    final String abi = _abi ?? 'arm64-v8a';
    final ByteData data = await rootBundle.load('assets/$abi/well-cli');
    final Directory tmp = await getApplicationSupportDirectory();
    final File exe = File('${tmp.path}/well-cli');
    await exe.writeAsBytes(data.buffer.asUint8List(), flush: true);
    await exe.setExecutable();
    return exe;
  }

  Future<void> _runCli([List<String> args = const []]) async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final exe = await _prepareExecutable();
      final ProcessResult result = await Process.run(exe.path, args);
      setState(() => _output = result.stdout + result.stderr);
    } catch (e) {
      setState(() => _output = 'Error: $e');
    } finally {
      setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Well CLI')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (_abi != null) Text('Detected ABI: $_abi'),
            Row(
              children: [
                Expanded(
                  child: ElevatedButton(
                    onPressed: _busy ? null : () => _runCli([]),
                    child: Text(_busy ? 'Running…' : 'Run well‑cli'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: ElevatedButton.icon(
                    onPressed: _busy ? null : () => _runCli(['image', 'synthwave neon grid wireframe sunset']),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: const Color(0xFF0284C7),
                      foregroundColor: Colors.white,
                    ),
                    icon: const Icon(Icons.image, size: 16),
                    label: const Text('🖼 Gen Image'),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Expanded(
              child: SingleChildScrollView(
                child: SelectableText(_output, style: const TextStyle(fontFamily: 'monospace')),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
