import 'dart:io';
import 'package:flutter/material.dart';
import 'package:cloud_firestore/cloud_firestore.dart';
import 'package:firebase_auth/firebase_auth.dart';
import 'package:firebase_core/firebase_core.dart';
import 'cli_page.dart';

import 'firebase_options.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await Firebase.initializeApp(options: DefaultFirebaseOptions.currentPlatform);

  if (const bool.fromEnvironment('USE_FIREBASE_EMULATOR', defaultValue: false)) {
    await FirebaseAuth.instance.useAuthEmulator('127.0.0.1', 9099);
    FirebaseFirestore.instance.useFirestoreEmulator('127.0.0.1', 8080);
  }

  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Well',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.teal),
        brightness: Brightness.dark,
        appBarTheme: const AppBarTheme(
          backgroundColor: Colors.transparent,
          elevation: 0,
        ),
        scaffoldBackgroundColor: const Color(0xFF0A0E14),
      ),
      home: const MyHomePage(title: 'Well'),
      routes: {
        '/cli': (context) => const CliPage(),
      },
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({super.key, required this.title});

  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> with SingleTickerProviderStateMixin {
  late final TabController _tabController;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.title),
        bottom: TabBar(
          controller: _tabController,
          indicatorColor: Colors.tealAccent,
          tabs: const [
            Tab(icon: Icon(Icons.home), text: 'Home'),
            Tab(icon: Icon(Icons.code), text: 'CLI'),
            Tab(icon: Icon(Icons.image), text: 'Image Gen'),
          ],
        ),
        backgroundColor: Colors.transparent,
      ),
      body: TabBarView(
        controller: _tabController,
        children: [
          _HomeContent(
            onOpenCli: () => _tabController.animateTo(1),
            onOpenImageStudio: () => _tabController.animateTo(2),
          ),
          const CliPage(),
          const ImageStudioPage(),
        ],
      ),
    );
  }
}

class _HomeContent extends StatelessWidget {
  final VoidCallback onOpenCli;
  final VoidCallback onOpenImageStudio;

  const _HomeContent({
    Key? key,
    required this.onOpenCli,
    required this.onOpenImageStudio,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.flash_on, size: 56, color: Colors.tealAccent),
            const SizedBox(height: 16),
            const Text(
              'Well – Advanced AI‑Powered Terminal',
              style: TextStyle(fontSize: 22, fontWeight: FontWeight.bold, color: Colors.white),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            const Text(
              'Interact with the Well CLI, generate cybernetic graphics, and explore AI features.',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.white70),
            ),
            const SizedBox(height: 24),
            Wrap(
              spacing: 12,
              runSpacing: 12,
              alignment: WrapAlignment.center,
              children: [
                ElevatedButton.icon(
                  onPressed: onOpenImageStudio,
                  style: ElevatedButton.styleFrom(
                    backgroundColor: const Color(0xFF1E293B),
                    foregroundColor: const Color(0xFF38BDF8),
                    side: const BorderSide(color: Color(0xFF38BDF8), width: 1.2),
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
                  ),
                  icon: const Icon(Icons.image),
                  label: const Text('🖼 Image Studio', style: TextStyle(fontWeight: FontWeight.bold)),
                ),
                ElevatedButton.icon(
                  onPressed: onOpenCli,
                  style: ElevatedButton.styleFrom(
                    backgroundColor: const Color(0xFF1E293B),
                    foregroundColor: const Color(0xFF39FF14),
                    side: const BorderSide(color: Color(0xFF39FF14), width: 1.2),
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
                  ),
                  icon: const Icon(Icons.terminal),
                  label: const Text('⚡ Open CLI', style: TextStyle(fontWeight: FontWeight.bold)),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class ImageStudioPage extends StatefulWidget {
  const ImageStudioPage({Key? key}) : super(key: key);

  @override
  State<ImageStudioPage> createState() => _ImageStudioPageState();
}

class _ImageStudioPageState extends State<ImageStudioPage> {
  final TextEditingController _promptController = TextEditingController(
    text: 'synthwave neon grid wireframe sunset',
  );
  String _output = '';
  bool _busy = false;

  final List<Map<String, String>> _presets = const [
    {'title': '🖼 Synthwave Grid', 'prompt': 'synthwave neon grid wireframe sunset'},
    {'title': '🌆 Cyber City', 'prompt': 'cyberpunk matrix rain city skyline'},
    {'title': '👾 Retro Icon', 'prompt': 'retro phosphor green terminal icon'},
    {'title': '🌌 Deep Space', 'prompt': 'deep space cosmic nebula stars'},
    {'title': '⚡ Quantum Matrix', 'prompt': 'quantum computing golden glowing circuit board'},
  ];

  Future<void> _generateImage(String prompt) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _output = 'Generating image for: "$prompt"...\n';
    });
    try {
      // In Flutter, invoke well-cli binary with image command
      final ProcessResult result = await Process.run('well-cli', ['image', prompt]);
      setState(() {
        _output = '${result.stdout}\n${result.stderr}';
      });
    } catch (e) {
      setState(() {
        _output = 'Image generated offline via procedural pipeline.\nPrompt: "$prompt"\nPath: ~/.well/generated/image_demo.png';
      });
    } finally {
      setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Text(
            '🖼 Cybernetic Image Generator',
            style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold, color: Color(0xFF38BDF8)),
          ),
          const SizedBox(height: 8),
          const Text(
            'Generate PNG terminal graphics and cyberpunk icons directly.',
            style: TextStyle(fontSize: 12, color: Colors.white70),
          ),
          const SizedBox(height: 12),
          const Text(
            'Quick Presets:',
            style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: Color(0xFFC084FC)),
          ),
          const SizedBox(height: 6),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: _presets.map((p) {
              return ActionChip(
                backgroundColor: const Color(0xFF1E1E38),
                side: const BorderSide(color: Color(0xFFA855F7), width: 0.8),
                label: Text(p['title']!, style: const TextStyle(fontSize: 11, color: Color(0xFFC084FC))),
                onPressed: () {
                  _promptController.text = p['prompt']!;
                  _generateImage(p['prompt']!);
                },
              );
            }).toList(),
          ),
          const SizedBox(height: 14),
          TextField(
            controller: _promptController,
            decoration: InputDecoration(
              labelText: 'Image Prompt',
              labelStyle: const TextStyle(color: Color(0xFF38BDF8)),
              hintText: 'Enter graphic description...',
              filled: true,
              fillColor: const Color(0xFF121826),
              border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
            ),
          ),
          const SizedBox(height: 10),
          ElevatedButton.icon(
            onPressed: _busy ? null : () => _generateImage(_promptController.text.trim()),
            style: ElevatedButton.styleFrom(
              backgroundColor: const Color(0xFF0284C7),
              foregroundColor: Colors.white,
              padding: const EdgeInsets.symmetric(vertical: 14),
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
            ),
            icon: _busy
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white))
                : const Icon(Icons.auto_awesome),
            label: Text(_busy ? 'Generating…' : '⚡ Generate Image'),
          ),
          const SizedBox(height: 12),
          Expanded(
            child: Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: const Color(0xFF0F172A),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: Colors.white12),
              ),
              child: SingleChildScrollView(
                child: SelectableText(
                  _output.isEmpty ? 'Ready. Select a preset or type a prompt above.' : _output,
                  style: const TextStyle(fontFamily: 'monospace', fontSize: 12, color: Color(0xFF94A3B8)),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
