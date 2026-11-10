// Standalone Dart checks, also runnable when Flutter's socket-based runner is unavailable.
import 'dart:convert';
import 'dart:io';

import 'package:maprisma_video_translator/i18n.dart';
import 'package:maprisma_video_translator/bridge.dart' show checkDetail;

void check(bool value, String message) {
  if (!value) throw StateError(message);
}

void main() {
  for (final entry in <(Map<String, String>, String)>[
    ({'LANG': 'de_DE.UTF-8'}, 'de'),
    ({'LC_ALL': '', 'LC_MESSAGES': 'fr_FR.UTF-8', 'LANG': 'de_DE'}, 'fr'),
    ({'LC_ALL': 'xx_YY.UTF-8', 'LC_MESSAGES': 'pl_PL'}, 'en'),
    ({'LC_ALL': 'C', 'LANG': 'it_IT'}, 'en'),
    ({'LANG': 'pt_BR.UTF-8'}, 'pt'),
    ({}, 'en'),
  ]) {
    check(
      systemLanguage(entry.$1) == entry.$2,
      'locale precedence: ${entry.$1}',
    );
  }
  final directory = Directory.systemTemp.createTempSync('maprisma-i18n-');
  try {
    final file = File('${directory.path}/interface.json');
    file.writeAsStringSync('{"other":42}');
    setInterfaceLanguage('de', settingsFile: file);
    check(tr('Start') == 'Starten', 'explicit selection');
    check(
      checkDetail({'detail': 'Start'}, {'model': 'Start'}) == 'Start',
      'model identifier preserved',
    );
    check(trf('System ({0})', ['Deutsch']) == 'System (Deutsch)', 'format');
    check(
      trf('Device {0}: {1}', ['name {1}', 'value']) == 'Device name {1}: value',
      'values containing braces',
    );
    check(tr('Uncatalogued text') == 'Uncatalogued text', 'English fallback');
    check(
      tr('Download Whisper small (480 MB)') ==
          'Whisper small herunterladen (480 MB)',
      'backend template',
    );
    check(
      tr('Use My Device') == 'My Device verwenden',
      'device name preserved',
    );
    check(
      tr(
        'Maprisma Video Translator needs a one-time setup: a private Python runtime with Whisper (about 300 MB).',
      ).contains('Laufzeitumgebung'),
      'nested setup text',
    );
    check(
      jsonDecode(file.readAsStringSync())['other'] == 42,
      'preserve settings',
    );
    check(
      jsonDecode(file.readAsStringSync())['language'] == 'de',
      'save selection',
    );
    setInterfaceLanguage('auto', settingsFile: file);
    check(
      jsonDecode(file.readAsStringSync())['language'] == 'auto',
      'save auto',
    );
    setInterfaceLanguage('invalid', settingsFile: file);
    check(interfaceLanguage == 'auto', 'reject invalid choice');
  } finally {
    directory.deleteSync(recursive: true);
    interfaceLanguage = 'auto';
  }
  stdout.writeln(
    'Language detection, selection, persistence, fallback and backend templates passed.',
  );
}
