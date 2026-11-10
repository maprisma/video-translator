import 'dart:convert';
import 'dart:io';

import 'translations.g.dart';

const languageNames = {
  'en': 'English',
  'de': 'Deutsch',
  'fr': 'Français',
  'es': 'Español',
  'it': 'Italiano',
  'pt': 'Português',
  'nl': 'Nederlands',
  'pl': 'Polski',
};
String systemLanguage([Map<String, String>? environment]) {
  final env = environment ?? Platform.environment;
  final locale =
      [
        env['LC_ALL'],
        env['LC_MESSAGES'],
        env['LANG'],
      ].whereType<String>().where((s) => s.isNotEmpty).firstOrNull ??
      '';
  final code = locale.split(RegExp(r'[_\.\-@]')).first.toLowerCase();
  return languageNames.containsKey(code) ? code : 'en';
}

File get interfaceSettingsFile {
  final env = Platform.environment;
  final config = env['XDG_CONFIG_HOME'];
  return File(
    '${config != null && config.isNotEmpty ? config : '${env['HOME'] ?? ''}/.config'}/maprisma-video-translator/interface.json',
  );
}

String _readLanguage() {
  try {
    final value = jsonDecode(
      interfaceSettingsFile.readAsStringSync(),
    )['language'];
    return value == 'auto' || languageNames.containsKey(value)
        ? value as String
        : 'auto';
  } catch (_) {
    return 'auto';
  }
}

String interfaceLanguage = _readLanguage();
void setInterfaceLanguage(String value, {File? settingsFile}) {
  if (value != 'auto' && !languageNames.containsKey(value)) return;
  final file = settingsFile ?? interfaceSettingsFile;
  file.parent.createSync(recursive: true);
  Map<String, dynamic> settings = {};
  try {
    settings = (jsonDecode(file.readAsStringSync()) as Map)
        .cast<String, dynamic>();
  } catch (_) {}
  settings['language'] = value;
  final temp = File('${file.path}.tmp');
  temp.writeAsStringSync(jsonEncode(settings));
  temp.renameSync(file.path);
  interfaceLanguage = value;
}

String tr(String english) {
  final code = interfaceLanguage == 'auto'
      ? systemLanguage()
      : interfaceLanguage;
  final catalog = catalogs[code];
  final exact = catalog?[english];
  if (exact != null) return exact;
  // Backend templates are matched only at presentation boundaries; values stay unchanged.
  final templates = (catalog ?? <String, String>{}).entries.toList()
    ..sort((a, b) => b.key.length.compareTo(a.key.length));
  for (final entry in templates) {
    if (!entry.key.contains('{0}')) continue;
    final slots = RegExp(r'\{\d+\}').allMatches(entry.key).toList();
    var pattern = '^';
    var offset = 0;
    for (final slot in slots) {
      pattern =
          '$pattern${RegExp.escape(entry.key.substring(offset, slot.start))}(.+?)';
      offset = slot.end;
    }
    final match = RegExp(
      '$pattern${RegExp.escape(entry.key.substring(offset))}\$',
      dotAll: true,
    ).firstMatch(english);
    if (match != null) {
      final values = <String, String>{};
      for (var i = 0; i < slots.length; i++) {
        final value = match.group(i + 1)!;
        values[slots[i].group(0)!] = _nested(entry.key, i) ? tr(value) : value;
      }
      return entry.value.replaceAllMapped(
        RegExp(r'\{\d+\}'),
        (slot) => values[slot.group(0)] ?? slot.group(0)!,
      );
    }
  }
  return english;
}

String trf(String english, List<Object?> values) =>
    tr(english).replaceAllMapped(RegExp(r'\{(\d+)\}'), (slot) {
      final index = int.parse(slot.group(1)!);
      return index < values.length ? '${values[index]}' : slot.group(0)!;
    });

bool _nested(String key, int index) =>
    key == 'Maprisma Video Translator needs a one-time setup: {0}.' ||
    key == '{0} and {1}' ||
    key == '{0} · worker stopped' ||
    key == 'Runtime setup failed: {0}' ||
    (key == '{0} is not installed or has no user service. {1}' && index == 1) ||
    (key == 'Enter the API key or bearer token for {0}.' && index == 0);
