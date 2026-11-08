import 'package:flutter_test/flutter_test.dart';
import 'package:maprisma_video_translator/bridge.dart';

void main() {
  test('bridge script path', () {
    expect(resolveBridgeScript({'VIDEO_TRANSLATOR_BACKEND': '/dev/backend/'}), '/dev/backend/bridge.sh');
    expect(resolveBridgeScript({'HOME': '/home/u'}), '/home/u/.local/share/maprisma-video-translator/backend/bridge.sh');
    expect(resolveBridgeScript({'HOME': '/home/u', 'XDG_DATA_HOME': '/data'}),
        '/data/maprisma-video-translator/backend/bridge.sh');
  });

  test('bridge output: the last line, errors as {error}', () {
    expect(parseBridgeOutput('noise\n{"status": "Ready"}\n'), {'status': 'Ready'});
    expect(parseBridgeOutput('not json').containsKey('error'), isTrue);
    expect(parseBridgeOutput('[1]').containsKey('error'), isTrue);
  });

  test('fix requests', () {
    expect(fixRequests({'label': 'Set up now', 'action': 'setup'}), [
      {'action': 'setup'}
    ]);
    expect(fixRequests({'settings': {'audio_input': 'Microphone', 'output': ''}}), [
      {'action': 'configure', 'settings': {'audio_input': 'Microphone', 'output': ''}},
      {'action': 'outputs'},
    ]);
  });

  test('next step', () {
    expect(nextStep({'setup': {'busy': true, 'message': ''}})!.text, 'Setting up…');
    expect(nextStep({'steps': [{'id': 'runtime', 'ok': true}]}), isNull);
    final step = nextStep({
      'setup': {'phase': 'error', 'message': 'uv failed.'},
      'steps': [
        {'id': 'runtime', 'ok': false, 'problem': 'Needs setup.', 'fixes': [{'label': 'Set up now', 'action': 'setup'}]}
      ],
    })!;
    expect(step.text, 'uv failed. Needs setup.');
    expect(step.fixes.single.label, 'Set up now');
    final provider = nextStep({
      'steps': [
        {'id': 'provider', 'ok': false, 'problem': 'Key?', 'fixes': [{'label': 'Enter API key…', 'panel': 'provider'}]}
      ],
    })!;
    expect(provider.fixes.single.panel, isTrue);
  });
}
