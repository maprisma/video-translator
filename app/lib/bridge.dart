// Requests to the caption controller through backend/bridge.sh: one JSON line in, the
// controller's state with the setup steps (or {"error": ...}) as the last line out. Port of the
// GNOME extension's bridge.js. The bridge starts the controller only for actions that need it
// and saves settings straight to the file while Maprisma Video Translator is off.

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'i18n.dart';

typedef Json = Map<String, dynamic>;

const languages = [
  'German', 'English', 'French', 'Spanish', 'Italian', 'Portuguese', 'Dutch', 'Polish', 'Ukrainian', 'Russian',
  'Japanese', 'Chinese', 'Korean', 'Arabic', 'Hindi', 'Turkish', 'Indonesian', 'Vietnamese',
];
List<(String, String)> get inputs => [
  ('Audio output', tr('Audio output · what you hear (video, calls)')),
  ('Microphone', tr('Microphone · speech in the room')),
];
List<(String, String)> get whisperModels => [
  ('tiny', tr('Tiny · 75 MB · fastest')),
  ('base', tr('Base · 145 MB')),
  ('small', tr('Small · 480 MB · recommended')),
  ('medium', tr('Medium · 1.5 GB')),
  ('large-v3', tr('Large v3 · 3 GB · most accurate')),
  ('turbo', tr('Turbo · 1.6 GB · large and fast')),
];
List<(String, String)> get sources => [
  ('auto', tr('Detect automatically')), ('en', tr('English')), ('de', tr('German')), ('fr', tr('French')), ('es', tr('Spanish')),
  ('it', tr('Italian')), ('ja', tr('Japanese')), ('zh', tr('Chinese')), ('ko', tr('Korean')), ('ru', tr('Russian')), ('ar', tr('Arabic')),
];
List<(String, String)> get providers => [
  ('llama.cpp', tr('llama.cpp · local')),
  ('Ollama', tr('Ollama · local')),
  ('Online', tr('Online · OpenAI-compatible server')),
];
const endpoints = {
  'llama.cpp': 'http://127.0.0.1:8080',
  'Ollama': 'http://127.0.0.1:11434',
  'Online': 'https://api.openai.com/v1',
};

/// Settings the controller still takes while captions run.
const liveSettings = {
  'font_size', 'caption_seconds', 'background_transparency', 'border_width', 'history_lines', 'auto_scroll',
  'text_color',
};

/// The bridge script: `$VIDEO_TRANSLATOR_BACKEND/bridge.sh` when that variable names the
/// backend directory, otherwise the installed `~/.local/share/maprisma-video-translator/backend`.
String resolveBridgeScript([Map<String, String>? environment]) {
  final env = environment ?? Platform.environment;
  final backend = env['VIDEO_TRANSLATOR_BACKEND'];
  if (backend != null && backend.isNotEmpty) {
    return '${backend.replaceFirst(RegExp(r'/+$'), '')}/bridge.sh';
  }
  final data = env['XDG_DATA_HOME']?.isNotEmpty == true ? env['XDG_DATA_HOME']! : '${env['HOME'] ?? ''}/.local/share';
  return '$data/maprisma-video-translator/backend/bridge.sh';
}

/// The last line of the bridge's output as a JSON object. Failures become {error}.
Json parseBridgeOutput(String out) {
  final last = out.trim().split('\n').last.trim();
  try {
    final value = jsonDecode(last.isEmpty ? '{}' : last);
    if (value is Map) return value.cast<String, dynamic>();
    return {'error': tr('Maprisma Video Translator: unexpected answer from the caption controller')};
  } on FormatException catch (error) {
    return {'error': trf('Maprisma Video Translator: {0}', [error.message])};
  }
}

class Bridge {
  Bridge({String? script}) : script = script ?? resolveBridgeScript();

  final String script;
  Future<void> _queue = Future.value();

  /// One request at a time, in order (like the popup's queue). Resolves to the controller's
  /// state with the setup steps; a failure resolves to {error}.
  Future<Json> request(Json payload) {
    final run = _queue.then((_) => _run(payload));
    _queue = run.then((_) {}, onError: (_) {});
    return run;
  }

  Future<Json> _run(Json payload) async {
    try {
      final process = await Process.start('bash', [script]);
      final stdoutText = process.stdout.transform(utf8.decoder).join();
      final stderrDone = process.stderr.drain<void>();
      process.stdin.write('${jsonEncode(payload)}\n');
      await process.stdin.close();
      final out = await stdoutText;
      await stderrDone;
      await process.exitCode;
      return parseBridgeOutput(out);
    } catch (error) {
      return {'error': trf('Maprisma Video Translator: {0}', [error is ProcessException ? error.message : error])};
    }
  }
}

/// A setup fix from readiness.steps(): {label, action?, settings?, panel?}.
class Fix {
  const Fix({required this.label, required this.requests, required this.panel});

  final String label;
  final List<Json> requests;

  /// The fix needs the translation server fields (an API key or model ID).
  final bool panel;
}

/// The requests a setup fix stands for, like bridge.js fixRequests.
List<Json> fixRequests(Map fix) {
  final requests = <Json>[];
  final settings = fix['settings'];
  if (settings is Map) requests.add({'action': 'configure', 'settings': settings});
  if (fix['action'] is String) requests.add({'action': fix['action']});
  if (settings is Map && settings.containsKey('audio_input')) requests.add({'action': 'outputs'});
  return requests;
}

class Step {
  const Step({required this.text, required this.fixes});

  final String text;
  final List<Fix> fixes;
}

/// What stands in the way of captions: the running setup, or the first failed setup step with
/// its fixes. Null when everything is ready.
Step? nextStep(Json state) {
  final setup = state['setup'] is Map ? state['setup'] as Map : const {};
  if (setup['busy'] == true) {
    final message = setup['message'];
    return Step(text: message is String && message.isNotEmpty ? message : tr('Setting up…'), fixes: const []);
  }
  final steps = state['steps'];
  if (steps is! List) return null;
  final failed = steps.whereType<Map>().where((step) => step['ok'] != true).firstOrNull;
  if (failed == null) return null;
  var problem = '${failed['problem'] ?? ''}';
  if (setup['phase'] == 'error' && failed['id'] == 'runtime') problem = '${tr('${setup['message']}')} ${tr(problem)}';
  final fixes = [
    for (final fix in (failed['fixes'] is List ? failed['fixes'] as List : const []).whereType<Map>())
      Fix(
        label: tr('${fix['label'] ?? ''}'),
        requests: fixRequests(fix),
        panel: fix['panel'] != null && fix['panel'] != false,
      ),
  ];
  return Step(text: tr(problem), fixes: fixes);
}

/// Startup checks that failed ({id, label, status, detail}).
List<Map> failedChecks(Json state) =>
    [for (final check in (state['checks'] is List ? state['checks'] as List : const []).whereType<Map>())
      if (check['status'] == 'error') check];

/// Backend check details can be a bare model name or language value.
String checkDetail(Map check, Json settings) {
  final detail = '${check['detail'] ?? ''}';
  if (['model', 'speech_model', 'target'].any((key) => settings[key] == detail)) return detail;
  return tr(detail);
}
