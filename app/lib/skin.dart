import 'package:flutter/material.dart';

import 'app_theme.dart';

const String skinKey = 'skin';
const List<String> appSkins = [
  'native',
  'synthwave',
  'cozy',
  'terminal',
  'disco',
  'zen',
];

@immutable
class AppSkinStyle {
  const AppSkinStyle(
    this.accent,
    this.borderTint,
    this.bgTint,
    this.windowBg,
    this.foreground,
    this.borderWidth,
  );

  final Color accent, borderTint, bgTint, windowBg, foreground;
  final double borderWidth;

  Map<String, String> toDesktopColors() => {
    'accent': colorHex(accent),
    'background': colorHex(windowBg),
    'foreground': colorHex(foreground),
  };
}

String normalizeSkin(String? raw) => appSkins.contains(raw) ? raw! : 'native';

AppSkinStyle? skinStyle(String? skin) => switch (normalizeSkin(skin)) {
  'synthwave' => const AppSkinStyle(
    Color(0xffff2e93),
    Color(0xff00e5ff),
    Color(0xff3a1259),
    Color(0xff130924),
    Color(0xfff4eaff),
    1.5,
  ),
  'cozy' => const AppSkinStyle(
    Color(0xfff09a36),
    Color(0xffe0822a),
    Color(0xff593012),
    Color(0xff21150e),
    Color(0xfff5e8d6),
    1.25,
  ),
  'terminal' => const AppSkinStyle(
    Color(0xff2ce868),
    Color(0xff22c55e),
    Color(0xff0d381a),
    Color(0xff09130b),
    Color(0xffd2fce0),
    1.25,
  ),
  'disco' => const AppSkinStyle(
    Color(0xffd946ef),
    Color(0xfffacc15),
    Color(0xff4c1666),
    Color(0xff1c0c2b),
    Color(0xfff8ecff),
    1.5,
  ),
  'zen' => const AppSkinStyle(
    Color(0xff52b798),
    Color(0xff64c2a4),
    Color(0xff1e3832),
    Color(0xff141d1c),
    Color(0xffe2f0ec),
    1.0,
  ),
  _ => null,
};

String skinLabel(String? skin, String Function(String) tr) =>
    tr(switch (normalizeSkin(skin)) {
      'synthwave' => 'Synthwave',
      'cozy' => 'Cozy',
      'terminal' => 'Terminal',
      'disco' => 'Disco',
      'zen' => 'Zen',
      _ => 'Native',
    });

/// Skin colours start opaque; withWindowAlpha applies the window's transparency afterwards.
ThemeData applySkinToTheme(ThemeData base, String? skin, {double? radius}) {
  final st = skinStyle(skin);
  if (st == null) return base;
  final sizes = base.extension<Corners>() ?? Corners(radius ?? 12);
  final r = radius ?? sizes.radius;
  final surfaceLow = Color.lerp(st.windowBg, st.bgTint, .28)!;
  final surface = Color.lerp(st.windowBg, st.bgTint, .45)!;
  final surfaceHigh = Color.lerp(st.windowBg, st.bgTint, .65)!;
  final scheme = base.colorScheme.copyWith(
    brightness: Brightness.dark,
    primary: st.accent,
    onPrimary: st.accent.computeLuminance() > .4 ? Colors.black : Colors.white,
    surface: st.windowBg,
    onSurface: st.foreground,
    onSurfaceVariant: Color.lerp(st.windowBg, st.foreground, .7),
    surfaceContainerLowest: Color.lerp(st.windowBg, st.bgTint, .14),
    surfaceContainerLow: surfaceLow,
    surfaceContainer: surface,
    surfaceContainerHigh: surfaceHigh,
    surfaceContainerHighest: Color.lerp(st.windowBg, st.foreground, .16),
    outlineVariant: st.borderTint.withValues(alpha: .42),
    secondaryContainer: Color.lerp(st.windowBg, st.accent, .22),
  );
  final card = Color.alphaBlend(st.bgTint.withValues(alpha: .28), st.windowBg);
  final side = BorderSide(
    color: st.borderTint.withValues(alpha: .48),
    width: st.borderWidth,
  );
  final menu = MenuStyle(
    backgroundColor: WidgetStatePropertyAll(scheme.surfaceContainerHigh),
  );
  return base.copyWith(
    colorScheme: scheme,
    scaffoldBackgroundColor: st.windowBg,
    canvasColor: st.windowBg,
    cardTheme: base.cardTheme.copyWith(
      color: card,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(r),
        side: side,
      ),
    ),
    dialogTheme: base.dialogTheme.copyWith(
      backgroundColor: scheme.surfaceContainerHigh,
    ),
    popupMenuTheme: base.popupMenuTheme.copyWith(
      color: scheme.surfaceContainerHigh,
    ),
    menuTheme: MenuThemeData(
      style: (base.menuTheme.style ?? const MenuStyle()).merge(menu),
    ),
    dropdownMenuTheme: DropdownMenuThemeData(
      menuStyle: (base.dropdownMenuTheme.menuStyle ?? const MenuStyle()).merge(
        menu,
      ),
    ),
    chipTheme: base.chipTheme.copyWith(
      backgroundColor: scheme.surfaceContainer,
      side: side.copyWith(width: 1),
      labelStyle: TextStyle(color: st.foreground),
    ),
    dividerTheme: base.dividerTheme.copyWith(color: scheme.outlineVariant),
  );
}
