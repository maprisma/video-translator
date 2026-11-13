import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:maprisma_video_translator/bridge.dart';
import 'package:maprisma_video_translator/translator_page.dart';

class FakeBridge extends Bridge {
  FakeBridge(this.state) : super(script: '/nonexistent');

  Json state;
  final sent = <Json>[];

  @override
  Future<Json> request(Json payload) async {
    sent.add(payload);
    return state;
  }
}

Json readyState({bool running = false}) => {
      'settings': {
        'audio_input': 'Audio output', 'output': 'hdmi', 'speech_model': 'small', 'source': 'auto',
        'target': 'German', 'provider': 'llama.cpp', 'model': 'gemma', 'endpoint': 'http://127.0.0.1:8080',
        'noise_filter': true, 'chunk_seconds': 5, 'auto_scroll': true, 'text_color': 'auto', 'caption_seconds': 5,
        'font_size': 24, 'background_transparency': -1, 'border_width': -1, 'history_lines': 500,
      },
      'outputs': [{'name': 'hdmi', 'label': 'HDMI'}],
      'models': ['gemma'],
      'steps': [{'id': 'runtime', 'ok': true}, {'id': 'device', 'ok': true}],
      'setup': {'busy': false},
      'checks': [],
      'text_colors': [{'value': 'auto', 'label': 'Automatic'}],
      'running': running,
      'overlay': running,
      'status': running ? 'Listening' : 'Ready',
      'shortcuts': true,
    };

Future<void> pump(WidgetTester tester, FakeBridge bridge) async {
  tester.view.physicalSize = const Size(1000, 3000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(MaterialApp(home: TranslatorPage(bridge: bridge, pollInterval: const Duration(hours: 1))));
  await tester.pumpAndSettle();
}

void main() {
  test('keyboard shortcut hints follow the desktop', () {
    final cosmic = shortcutsHint({'XDG_CURRENT_DESKTOP': 'COSMIC', 'HOME': '/h'});
    expect(cosmic, contains('COSMIC Settings'));
    final kde = shortcutsHint({'XDG_CURRENT_DESKTOP': 'KDE', 'HOME': '/h'});
    expect(kde, contains('System Settings'));
    expect(kde, contains('/h/.local/bin/video-trans toggle'));
    expect(kde, isNot(contains('COSMIC')));
    expect(shortcutsHint({'XDG_CURRENT_DESKTOP': 'Hyprland', 'HOME': '/h'}), contains('Hyprland configuration'));
    expect(shortcutsHint({'XDG_CURRENT_DESKTOP': 'GNOME', 'HOME': '/h'}), isNot(contains('COSMIC')));
  });

  testWidgets('ready: Start sends start', (tester) async {
    final bridge = FakeBridge(readyState());
    await pump(tester, bridge);
    expect(find.text('Ready'), findsOneWidget);
    expect(find.text('Not ready yet'), findsNothing);
    expect(find.text('Corners'), findsOneWidget);
    expect(find.text('F12 saves a screenshot'), findsOneWidget);
    await tester.tap(find.text('Start'));
    await tester.pumpAndSettle();
    expect(bridge.sent.last, {'action': 'start'});
  });

  testWidgets('a setup step shows its fixes', (tester) async {
    final state = readyState()
      ..['steps'] = [
        {'id': 'runtime', 'ok': false, 'problem': 'Maprisma Video Translator needs a one-time setup.',
         'fixes': [{'label': 'Set up now', 'action': 'setup'}]}
      ];
    final bridge = FakeBridge(state);
    await pump(tester, bridge);
    expect(find.text('Not ready yet'), findsOneWidget);
    await tester.tap(find.text('Set up now'));
    await tester.pumpAndSettle();
    expect(bridge.sent.last, {'action': 'setup'});
  });

  testWidgets('while running, only live settings are enabled', (tester) async {
    final bridge = FakeBridge(readyState(running: true));
    await pump(tester, bridge);
    expect(find.text('Listening'), findsWidgets);
    final start = tester.widget<FilledButton>(find.ancestor(of: find.text('Start'), matching: find.byWidgetPredicate((w) => w is FilledButton)));
    expect(start.onPressed, isNull);
    await tester.tap(find.text('Stop'));
    await tester.pumpAndSettle();
    expect(bridge.sent.last, {'action': 'stop'});
  });
}
