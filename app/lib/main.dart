// Maprisma Video Translator for the COSMIC desktop: the window with the controls and all settings, a
// Flutter port of the GNOME extension's settings window. It talks only to the backend's
// bridge.sh; the panel applet starts it ("Open Maprisma Video Translator…").

import 'package:flutter/material.dart';

import 'bridge.dart';
import 'i18n.dart';
import 'desktop_theme.dart';
import 'look.dart';
import 'shot.dart';
import 'translator_page.dart';

void main() {
  runApp(TranslatorApp(bridge: Bridge()));
}

class TranslatorApp extends StatefulWidget {
  const TranslatorApp({super.key, required this.bridge, this.desktopTheme});

  final Bridge bridge;
  final DesktopTheme? desktopTheme;

  @override
  State<TranslatorApp> createState() => _TranslatorAppState();
}

class _TranslatorAppState extends State<TranslatorApp> {
  late final DesktopTheme _desktop = widget.desktopTheme ?? DesktopTheme();

  @override
  void initState() {
    super.initState();
    _desktop.start();
  }

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<LookSettings>(
      valueListenable: look,
      builder: (context, settings, _) => ValueListenableBuilder<Color?>(
        valueListenable: _desktop.accent,
        builder: (context, accent, _) => MaterialApp(
          title: tr('Maprisma Video Translator'),
          debugShowCheckedModeBanner: false,
          navigatorKey: shotNavigator,
          scaffoldMessengerKey: shotMessenger,
          themeMode: ThemeMode.system,
          theme: DesktopTheme.theme(Brightness.light, accent,
              radius: cornerRadius(settings.corners, desktopRadius(Brightness.light))),
          darkTheme: DesktopTheme.theme(Brightness.dark, accent,
              radius: cornerRadius(settings.corners, desktopRadius(Brightness.dark))),
          builder: (context, child) {
            _desktop.syncHeader(Theme.of(context));
            return ShotScope(child: child!);
          },
          home: TranslatorPage(bridge: widget.bridge),
        ),
      ),
    );
  }
}
