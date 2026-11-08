// The Maprisma Video Translator window: the controls (Start / Stop / Captions), what needs setting up,
// then Listening, Translation, Translation server and Captions (the groups of the Omarchy popup
// and the GNOME settings window). Every change goes to the controller at once through the
// bridge, or straight to the settings file while Maprisma Video Translator is off. The state is polled
// every 1.5 s while the window is open.

import 'dart:async';
import 'dart:ui' show AppExitResponse;

import 'package:flutter/material.dart';

import 'bridge.dart';
import 'widgets.dart';

/// A number as JavaScript prints it (800, not 800.0).
String jsNumber(Object? value) {
  if (value is num && value == value.roundToDouble() && value.isFinite) return value.toInt().toString();
  return '$value';
}

/// The choices, with the current value added when the list does not know it.
List<Option> withCurrent(List<Option> options, String? value, [String Function(String)? label]) {
  if (value == null || value.isEmpty || options.any((o) => o.$1 == value)) return options;
  return [...options, (value, label?.call(value) ?? value)];
}

class TranslatorPage extends StatefulWidget {
  const TranslatorPage({super.key, required this.bridge, this.pollInterval = const Duration(milliseconds: 1500)});

  final Bridge bridge;
  final Duration pollInterval;

  @override
  State<TranslatorPage> createState() => _TranslatorPageState();
}

class _TranslatorPageState extends State<TranslatorPage> {
  Json _state = {};
  Timer? _timer;
  bool _polling = false;
  late final AppLifecycleListener _lifecycle;

  final _endpoint = TextEditingController();
  final _modelId = TextEditingController();
  final _token = TextEditingController();
  final _endpointFocus = FocusNode();
  final _modelFocus = FocusNode();
  final _serverKey = GlobalKey();
  final _scroll = ScrollController();

  // Number fields wait 400 ms after the last change before they are saved.
  final Map<String, double> _pending = {};
  final Map<String, Timer> _saveTimers = {};

  @override
  void initState() {
    super.initState();
    _refresh();
    _timer = Timer.periodic(widget.pollInterval, (_) => _refresh());
    // Closing the window: a controller without captions on screen exits, as when the Omarchy
    // popup closed.
    _lifecycle = AppLifecycleListener(onExitRequested: () async {
      await widget.bridge.request({'action': 'release'}).timeout(const Duration(seconds: 3), onTimeout: () => {});
      return AppExitResponse.exit;
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    for (final timer in _saveTimers.values) {
      timer.cancel();
    }
    _lifecycle.dispose();
    for (final c in [_endpoint, _modelId, _token]) {
      c.dispose();
    }
    _endpointFocus.dispose();
    _modelFocus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Json get _settings => (_state['settings'] as Map?)?.cast<String, dynamic>() ?? {};

  Future<void> _send(Json payload) async {
    final state = await widget.bridge.request(payload);
    if (!mounted) return;
    if (state['error'] != null) {
      ScaffoldMessenger.of(context)
        ..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text('${state['error']}'), duration: const Duration(seconds: 6)));
    } else {
      _update(state);
    }
  }

  void _configure(Json settings) => _send({'action': 'configure', 'settings': settings});

  Future<void> _refresh() async {
    // Skip a poll while one is still on its way, so the controls do not jump back.
    if (_polling) return;
    _polling = true;
    // hold: the controller stays while this window is open, even when the panel menu closes.
    final state = await widget.bridge.request({'action': 'state', 'hold': true});
    _polling = false;
    if (mounted && state['error'] == null) _update(state);
  }

  void _update(Json state) {
    if (state['settings'] is! Map) return;
    setState(() {
      _state = state;
      final settings = _settings;
      // Refreshes leave a field alone while the cursor is in it.
      if (!_endpointFocus.hasFocus) _endpoint.text = '${settings['endpoint'] ?? ''}';
      if (!_modelFocus.hasFocus) _modelId.text = '${settings['model'] ?? ''}';
    });
  }

  void _applyFix(Fix fix) {
    for (final request in fix.requests) {
      _send(request);
    }
    if (fix.panel) {
      final target = _serverKey.currentContext;
      if (target != null) Scrollable.ensureVisible(target, duration: const Duration(milliseconds: 300));
      (_settings['provider'] == 'Online' && '${_settings['endpoint']}'.isNotEmpty ? _modelFocus : _endpointFocus)
          .requestFocus();
    }
  }

  void _number(String key, double value) {
    setState(() => _pending[key] = value);
    _saveTimers[key]?.cancel();
    _saveTimers[key] = Timer(const Duration(milliseconds: 400), () {
      _saveTimers.remove(key);
      _pending.remove(key);
      _configure({key: value.round()});
    });
  }

  // ------------------------------------------------------------------- state

  List<Map> get _steps => (_state['steps'] is List ? _state['steps'] as List : const []).whereType<Map>().toList();
  bool get _loaded => _state['settings'] is Map;
  bool get _runtimeReady => _loaded && !_steps.any((s) => s['id'] == 'runtime' && s['ok'] != true);
  bool get _ready => _loaded && _steps.isNotEmpty && _steps.every((s) => s['ok'] == true);
  bool get _running => _state['running'] == true;
  bool get _locked =>
      _running || _state['starting'] == true || _state['downloading'] == true || _state['stopping'] == true;

  bool _enabled(String key) => !_locked || liveSettings.contains(key);

  // ------------------------------------------------------------------- build

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: !_loaded
          ? const Center(child: CircularProgressIndicator())
          : Scrollbar(
              controller: _scroll,
              child: ListView(
                controller: _scroll,
                padding: const EdgeInsets.fromLTRB(20, 20, 20, 8),
                children: [
                  _statusGroup(),
                  ?_stepGroup(),
                  _listeningGroup(),
                  _translationGroup(),
                  _serverGroup(),
                  _captionsGroup(),
                  _appearanceGroup(),
                  _shortcutsGroup(),
                ],
              ),
            ),
    );
  }

  Widget _statusGroup() {
    final theme = Theme.of(context);
    final setup = _state['setup'] is Map ? _state['setup'] as Map : const {};
    final setupMessage = setup['busy'] == true || setup['phase'] == 'error' ? '${setup['message'] ?? ''}' : '';
    final status = setupMessage.isNotEmpty ? setupMessage : '${_state['status'] ?? ''}';
    final stopping = _state['stopping'] == true;
    final checks = (_state['checks'] is List ? _state['checks'] as List : const []).whereType<Map>().toList();
    final anyError = checks.any((c) => c['status'] == 'error');
    return PrefGroup(children: [
      Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.subtitles_outlined, color: _running ? Colors.green : theme.colorScheme.primary),
                const SizedBox(width: 10),
                Expanded(child: Text('Maprisma Video Translator', style: theme.textTheme.titleMedium)),
                FilledButton.icon(
                  onPressed: _ready && !_running && !stopping && _state['downloading'] != true && setup['busy'] != true
                      ? () => _send({'action': 'start'})
                      : null,
                  icon: const Icon(Icons.play_arrow),
                  label: const Text('Start'),
                ),
                const SizedBox(width: 6),
                OutlinedButton.icon(
                  onPressed: (_running || _state['overlay'] == true) && !stopping ? () => _send({'action': 'stop'}) : null,
                  icon: const Icon(Icons.stop),
                  label: const Text('Stop'),
                ),
                const SizedBox(width: 6),
                OutlinedButton(
                  onPressed: _runtimeReady ? () => _send({'action': 'show'}) : null,
                  child: const Text('Captions'),
                ),
              ],
            ),
            if (status.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(status,
                    style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
              ),
          ],
        ),
      ),
      for (final check in checks)
        if (check['status'] != 'ok' || anyError)
          PrefRow(
            title: '${{'ok': '✓', 'error': '✕'}[check['status']] ?? '…'} ${check['label'] ?? ''}',
            subtitle: '${check['detail'] ?? ''}',
            error: check['status'] == 'error',
          ),
    ]);
  }

  Widget? _stepGroup() {
    final step = nextStep(_state);
    if (step == null) return null;
    return PrefGroup(title: 'Not ready yet', children: [
      PrefRow(title: step.text),
      for (final (index, fix) in step.fixes.indexed)
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
          child: index == 0
              ? FilledButton(onPressed: () => _applyFix(fix), child: Text(fix.label))
              : OutlinedButton(onPressed: () => _applyFix(fix), child: Text(fix.label)),
        ),
    ]);
  }

  Widget _listeningGroup() {
    final settings = _settings;
    final devices = <Option>[
      for (final d in (_state['outputs'] is List ? _state['outputs'] as List : const []).whereType<Map>())
        ('${d['name']}', '${d['label']}'),
    ];
    final output = '${settings['output'] ?? ''}';
    final downloading = _state['downloading'] == true;
    return PrefGroup(title: 'Listening', children: [
      PrefRow(title: 'Capture', enabled: !_locked, suffixes: [
        ChoiceButton(
          options: inputs,
          value: settings['audio_input'] as String?,
          onPick: (value) {
            _configure({'audio_input': value, 'output': ''});
            _send({'action': 'outputs'});
          },
        ),
      ]),
      PrefRow(title: 'Device', enabled: !_locked, suffixes: [
        TextButton(onPressed: () => _send({'action': 'outputs'}), child: const Text('Refresh')),
        ChoiceButton(
          options: withCurrent(devices, output, (name) => '$name (not connected)'),
          value: output,
          onPick: (value) => _configure({'output': value}),
        ),
      ]),
      PrefRow(
        title: 'Speech recognition',
        subtitle: 'Whisper model, runs on the CPU',
        enabled: !_locked || downloading,
        suffixes: [
          OutlinedButton(
            onPressed: _runtimeReady && (downloading || !_locked)
                ? () => _send({'action': downloading ? 'cancel-download' : 'download'})
                : null,
            child: Text(downloading ? 'Cancel' : 'Download'),
          ),
          Sensitive(
            enabled: !_locked,
            child: ChoiceButton(
              options: withCurrent(whisperModels, settings['speech_model'] as String?),
              value: settings['speech_model'] as String?,
              onPick: (value) => _configure({'speech_model': value}),
            ),
          ),
        ],
      ),
      PrefRow(title: 'Spoken language', subtitle: 'Override when detection guesses wrong', enabled: !_locked, suffixes: [
        ChoiceButton(
          options: withCurrent(sources, settings['source'] as String?),
          value: settings['source'] as String?,
          onPick: (value) => _configure({'source': value}),
        ),
      ]),
      _switch('noise_filter', 'Noise suppression', 'Speech-band filter and adaptive noise reduction'),
      _spin('chunk_seconds', 'Audio chunk (seconds)', 3, 15, 1,
          'Shorter chunks show captions sooner; longer ones give Whisper more context'),
    ]);
  }

  Widget _translationGroup() {
    final settings = _settings;
    final model = '${settings['model'] ?? ''}';
    final models = <String>{
      for (final m in (_state['models'] is List ? _state['models'] as List : const [])) '$m',
      if (model.isNotEmpty) model,
    };
    final canFind = _runtimeReady && !_locked;
    return PrefGroup(title: 'Translation', children: [
      PrefRow(title: 'Translate into', enabled: !_locked, suffixes: [
        ChoiceButton(
          title: 'Translate into',
          searchable: true,
          options: [for (final l in languages) (l, l)],
          value: settings['target'] as String?,
          onPick: (value) => _configure({'target': value}),
        ),
      ]),
      PrefRow(title: 'Translator', enabled: !_locked, suffixes: [
        ChoiceButton(
          options: providers,
          value: settings['provider'] as String?,
          onPick: (value) => _configure({'provider': value, 'model': '', 'endpoint': endpoints[value]}),
        ),
      ]),
      PrefRow(title: 'Model', enabled: !_locked, suffixes: [
        ChoiceButton(
          title: 'Model',
          searchable: true,
          options: [for (final m in models) (m, m)],
          value: model,
          onPick: (value) => _configure({'model': value}),
        ),
      ]),
      PrefRow(
        title: 'Find a model',
        subtitle: 'Choose automatically picks an installed text model for the target language (by name, not a benchmark)',
        suffixes: [
          if (settings['provider'] != 'Online')
            OutlinedButton(
                onPressed: canFind ? () => _send({'action': 'suggest'}) : null, child: const Text('Choose automatically')),
          OutlinedButton(onPressed: canFind ? () => _send({'action': 'models'}) : null, child: const Text('Load model list')),
        ],
      ),
    ]);
  }

  Widget _serverGroup() {
    final keyReady = _state['key_ready'] == true;
    void saveToken() {
      if (_token.text.isEmpty) return;
      _send({'action': 'configure', 'token': _token.text, 'remember_token': _state['remember_token'] == true});
      _token.clear();
    }

    return KeyedSubtree(
      key: _serverKey,
      child: PrefGroup(
        title: 'Translation server',
        description: 'Only recognised text is sent to it, never audio. Online providers may charge your account.',
        children: [
          _entry('Server address', _endpoint, _endpointFocus, () => _configure({'endpoint': _endpoint.text.trim()})),
          _entry('Model ID', _modelId, _modelFocus, () => _configure({'model': _modelId.text.trim()})),
          EntryPrefRow(
            title: keyReady ? 'API key or bearer token · set' : 'API key or bearer token',
            controller: _token,
            password: true,
            onSubmitted: (_) => saveToken(),
            suffixes: [TextButton(onPressed: saveToken, child: const Text('Apply'))],
          ),
          SwitchPrefRow(
            title: 'Save token on this device',
            subtitle: 'An owner-only file, not a keychain',
            value: _state['remember_token'] == true,
            onChanged: (on) => _send({'action': 'configure', 'remember_token': on}),
          ),
        ],
      ),
    );
  }

  Widget _entry(String title, TextEditingController controller, FocusNode focus, VoidCallback apply) {
    return Sensitive(
      enabled: !_locked,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 4, 14, 4),
        child: Row(children: [
          Expanded(
            child: TextField(
              controller: controller,
              focusNode: focus,
              autocorrect: false,
              onSubmitted: (_) => apply(),
              decoration: InputDecoration(labelText: title, border: InputBorder.none, isDense: true),
            ),
          ),
          TextButton(onPressed: apply, child: const Text('Apply')),
        ]),
      ),
    );
  }

  Widget _captionsGroup() {
    final settings = _settings;
    final colors = <Option>[
      for (final o in (_state['text_colors'] is List ? _state['text_colors'] as List : const []).whereType<Map>())
        ('${o['value']}', '${o['label']}'),
    ];
    return PrefGroup(title: 'Captions', children: [
      _switch('auto_scroll', 'Follow new captions', 'Scroll to each new caption'),
      PrefRow(title: 'Caption text colour', subtitle: 'The backdrop adapts for contrast', suffixes: [
        ChoiceButton(
          options: colors,
          value: settings['text_color'] as String?,
          onPick: (value) => _configure({'text_color': value}),
        ),
      ]),
      _spin('caption_seconds', 'Show each caption for (seconds)', 1, 120, 1),
      _spin('font_size', 'Caption font size', 12, 64, 1),
      PrefRow(
        title: 'Caption window',
        subtitle: 'Drag it with the mouse; resize it with the handle at its bottom-right corner',
        suffixes: [
          OutlinedButton(
              onPressed: _runtimeReady ? () => _send({'action': 'clear'}) : null, child: const Text('Clear captions')),
          OutlinedButton(
              onPressed: _state['overlay'] == true ? () => _send({'action': 'close'}) : null, child: const Text('Close')),
        ],
      ),
    ]);
  }

  Widget _appearanceGroup() {
    return PrefGroup(
      title: 'Caption window appearance',
      description: 'Only the caption window is affected. −1 uses the theme default. Lower the transparency so the '
          'backdrop behind black or white text shows.',
      children: [
        _spin('background_transparency', 'Background transparency (%)', -1, 100, 5),
        _spin('border_width', 'Border thickness (px)', -1, 16, 1),
        _spin('history_lines', 'Retained caption paragraphs', 50, 5000, 50),
      ],
    );
  }

  Widget _shortcutsGroup() {
    return PrefGroup(
      title: 'Keyboard shortcuts',
      description: 'In COSMIC Settings → Input Devices → Keyboard → Keyboard shortcuts → Custom shortcuts, add a '
          'shortcut with a command such as “video-trans toggle”, “video-trans font bigger” or '
          '“video-trans text-color next”. “video-trans help” in a terminal lists every action.',
      children: [
        if (_state['shortcuts'] == false && _runtimeReady)
          Padding(
            padding: const EdgeInsets.all(10),
            child: OutlinedButton(
              onPressed: () => _send({'action': 'install-shortcuts'}),
              child: const Text('Add the video-trans command'),
            ),
          )
        else
          const PrefRow(title: 'The video-trans command is installed'),
      ],
    );
  }

  Widget _switch(String key, String title, [String subtitle = '']) {
    return SwitchPrefRow(
      title: title,
      subtitle: subtitle,
      value: _settings[key] == true,
      enabled: _enabled(key),
      onChanged: (on) => _configure({key: on}),
    );
  }

  Widget _spin(String key, String title, double lower, double upper, double step, [String subtitle = '']) {
    final value = _pending[key] ?? (_settings[key] is num ? (_settings[key] as num).toDouble() : lower);
    return PrefRow(title: title, subtitle: subtitle, enabled: _enabled(key), suffixes: [
      SpinField(
        value: value.clamp(lower, upper).toDouble(),
        lower: lower,
        upper: upper,
        step: step,
        digits: 0,
        onChanged: (v) => _number(key, v),
      ),
    ]);
  }
}
