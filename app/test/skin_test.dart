import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:maprisma_video_translator/app_theme.dart';
import 'package:maprisma_video_translator/bridge.dart';
import 'package:maprisma_video_translator/look.dart';
import 'package:maprisma_video_translator/main.dart';
import 'package:maprisma_video_translator/skin.dart';
import 'package:maprisma_video_translator/translator_page.dart';
import 'package:maprisma_ui/window_style.dart';

class _Bridge extends Bridge {
  _Bridge() : super(script: '/nonexistent');
  @override
  Future<Json> request(Json payload) async => {
    'settings': {},
    'steps': [],
    'setup': {},
    'checks': [],
    'status': 'Ready',
  };
}

void main() {
  test('names, exact palettes and desktop colors', () {
    expect(appSkins, [
      'native',
      'synthwave',
      'cozy',
      'terminal',
      'disco',
      'zen',
    ]);
    expect(normalizeSkin(null), 'native');
    expect(normalizeSkin('other'), 'native');
    expect(skinStyle('native'), isNull);
    const expected = [
      (
        'synthwave',
        0xffff2e93,
        0xff00e5ff,
        0xff3a1259,
        0xff130924,
        0xfff4eaff,
        1.5,
      ),
      (
        'cozy',
        0xfff09a36,
        0xffe0822a,
        0xff593012,
        0xff21150e,
        0xfff5e8d6,
        1.25,
      ),
      (
        'terminal',
        0xff2ce868,
        0xff22c55e,
        0xff0d381a,
        0xff09130b,
        0xffd2fce0,
        1.25,
      ),
      (
        'disco',
        0xffd946ef,
        0xfffacc15,
        0xff4c1666,
        0xff1c0c2b,
        0xfff8ecff,
        1.5,
      ),
      ('zen', 0xff52b798, 0xff64c2a4, 0xff1e3832, 0xff141d1c, 0xffe2f0ec, 1.0),
    ];
    for (final (name, accent, border, bg, window, foreground, width)
        in expected) {
      final st = skinStyle(name)!;
      expect(
        [
          st.accent.toARGB32(),
          st.borderTint.toARGB32(),
          st.bgTint.toARGB32(),
          st.windowBg.toARGB32(),
          st.foreground.toARGB32(),
          st.borderWidth,
        ],
        [accent, border, bg, window, foreground, width],
      );
      expect(st.toDesktopColors(), {
        'accent': colorHex(st.accent),
        'background': colorHex(st.windowBg),
        'foreground': colorHex(st.foreground),
      });
      expect(skinLabel(name, (s) => s), isNotEmpty);
    }
  });

  test('skin theme keeps extensions and composes with window transparency', () {
    final base = appTheme(
      Brightness.light,
      const {},
      6,
      fonts: {'ui': 'Inter', 'mono': 'Hack'},
    );
    expect(identical(applySkinToTheme(base, 'native'), base), isTrue);
    final st = skinStyle('synthwave')!;
    final themed = applySkinToTheme(base, 'synthwave', radius: 6);
    expect(themed.colorScheme.brightness, Brightness.dark);
    expect(themed.colorScheme.primary, st.accent);
    expect(themed.scaffoldBackgroundColor, st.windowBg);
    expect(
      themed.cardTheme.color,
      Color.alphaBlend(st.bgTint.withValues(alpha: .28), st.windowBg),
    );
    expect((themed.cardTheme.shape as RoundedRectangleBorder).side.width, 1.5);
    expect(themed.extension<Corners>()!.radius, 6);
    expect(themed.extension<AppFonts>()!.mono, 'Hack');
    expect(themed.extension<StateColors>(), isNotNull);
    final glass = withWindowAlpha(themed, .6);
    expect(glass.scaffoldBackgroundColor.a, closeTo(.6, .001));
    expect(glass.colorScheme.surfaceContainerLow.a, closeTo(.8, .001));
    expect(glass.dialogTheme.backgroundColor!.a, 1);
    expect(glass.popupMenuTheme.color!.a, 1);
    expect(glass.menuTheme.style!.backgroundColor!.resolve({})!.a, 1);
  });

  test('persistence omits native and preserves other settings', () {
    final dir = Directory.systemTemp.createTempSync('skin-test-');
    addTearDown(() => dir.deleteSync(recursive: true));
    final file = File('${dir.path}/interface.json')
      ..writeAsStringSync(
        '{"language":"de","corners":"square","skin":"invalid"}',
      );
    expect(readLook(file).skin, 'native');
    saveLook(skin: 'disco', file: file);
    expect(readLook(file).skin, 'disco');
    expect(jsonDecode(file.readAsStringSync()), {
      'language': 'de',
      'corners': 'square',
      'skin': 'disco',
    });
    saveLook(skin: 'native', file: file);
    expect(jsonDecode(file.readAsStringSync()), {
      'language': 'de',
      'corners': 'square',
    });
  });

  testWidgets('settings selection saves and applies skin immediately', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('skin-ui-');
    final file = File('${dir.path}/interface.json');
    final previous = lookSettingsFile;
    lookSettingsFile = () => file;
    look.value = const LookSettings();
    addTearDown(() {
      lookSettingsFile = previous;
      look.value = const LookSettings();
      dir.deleteSync(recursive: true);
    });
    tester.view.physicalSize = const Size(1000, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(TranslatorApp(bridge: _Bridge(), onClose: () {}));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('App skin').first);
    await tester.tap(find.text('Native').first);
    await tester.pumpAndSettle();
    expect(find.text('Disco'), findsWidgets);
    await tester.tap(find.text('Disco').last);
    await tester.pumpAndSettle();
    expect(look.value.skin, 'disco');
    expect(jsonDecode(file.readAsStringSync())['skin'], 'disco');
    expect(Theme.of(tester.element(find.byType(TranslatorPage))).colorScheme.primary, skinStyle('disco')!.accent);
  });
}
