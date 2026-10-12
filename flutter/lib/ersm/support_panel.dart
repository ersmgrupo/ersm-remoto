// ERSM Desk support mode: the whole content of the compact window (S16 start, S17 connected,
// S19 finished). Texts and states follow replica/design/components.md, §6; colours come only
// from tokens.g.dart. The state comes from Rust (src/ersm/support.rs, status_json).
import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:window_manager/window_manager.dart';

import 'tokens.g.dart';

class ErsmSupportStatus {
  final bool support;
  final String phase;
  final String detail;
  final String tech;

  const ErsmSupportStatus(this.support, this.phase, this.detail, this.tech);

  static ErsmSupportStatus read() {
    try {
      final m = jsonDecode(bind.mainGetErsmSupportStatus()) as Map<String, dynamic>;
      return ErsmSupportStatus(m['support'] == true, '${m['phase'] ?? ''}',
          '${m['detail'] ?? ''}', '${m['tech'] ?? ''}');
    } catch (_) {
      return const ErsmSupportStatus(false, '', '', '');
    }
  }
}

bool? _isSupportMode;

bool ersmIsSupportMode() =>
    _isSupportMode ??= ErsmSupportStatus.read().support;

class ErsmSupportPanel extends StatefulWidget {
  const ErsmSupportPanel({Key? key}) : super(key: key);

  @override
  State<ErsmSupportPanel> createState() => _ErsmSupportPanelState();
}

class _ErsmSupportPanelState extends State<ErsmSupportPanel> {
  ErsmSupportStatus _status = ErsmSupportStatus.read();
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      final next = ErsmSupportStatus.read();
      if (next.phase != _status.phase ||
          next.detail != _status.detail ||
          next.tech != _status.tech) {
        setState(() => _status = next);
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  bool get _dark => Theme.of(context).brightness == Brightness.dark;
  Color get _text => _dark ? ErsmDark.text : ErsmLight.text;
  Color get _muted => _dark ? ErsmDark.textMuted : ErsmLight.textMuted;

  String get _tech => _status.tech.isEmpty ? 'IT de ERSM' : _status.tech;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      container: true,
      liveRegion: true,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
            ErsmSpace.s5, ErsmSpace.s5, ErsmSpace.s5, ErsmSpace.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            _header(),
            const SizedBox(height: ErsmSpace.s5),
            ..._body(),
          ],
        ),
      ),
    );
  }

  Widget _header() {
    return Container(
      padding: const EdgeInsets.symmetric(
          horizontal: ErsmSpace.s4, vertical: ErsmSpace.s3),
      decoration: BoxDecoration(
        color: ErsmLight.brandStrong,
        borderRadius: BorderRadius.circular(ErsmRadius.md),
      ),
      child: Text.rich(
        TextSpan(children: const [
          TextSpan(text: 'ERSM', style: TextStyle(fontWeight: FontWeight.w700)),
          TextSpan(text: ' Desk · Soporte'),
        ]),
        style: const TextStyle(
            color: ErsmLight.onBrandStrong, fontSize: ErsmText.lgSize),
      ),
    );
  }

  List<Widget> _body() {
    switch (_status.phase) {
      case 'connected':
        return [
          _banner('$_tech (IT de ERSM) está conectado a tu equipo.'),
          const SizedBox(height: ErsmSpace.s3),
          _paragraph(
              'Puede ver tu pantalla y usar tu ratón y teclado. Para cortar, pulsa «Desconectar» en la ventana de la conexión.'),
        ];
      case 'ended':
        return [
          _title('La sesión ha terminado'),
          _paragraph(
              'Este programa no se queda en tu equipo. Puedes cerrarlo y borrar el archivo descargado.'),
          const SizedBox(height: ErsmSpace.s4),
          _closeButton(primary: true),
        ];
      case 'waiting':
        return [
          _title('Listo'),
          _paragraph('$_tech se va a conectar en un momento.'),
          _paragraph(
              'Cuando aparezca la ventana de permiso, comprueba que es la persona con la que estás hablando y pulsa «Aceptar».'),
        ];
      case 'error':
        return [
          _message(_status.detail.isEmpty
              ? 'No se ha podido conectar con IT.'
              : _status.detail),
          const SizedBox(height: ErsmSpace.s4),
          _closeButton(primary: false),
        ];
      case 'no_ticket':
        return [
          _message(
              'Este programa se abre desde la página de soporte. Vuelve a escribir el código que te ha dado IT en remoto.ersmgrupo.org.'),
          const SizedBox(height: ErsmSpace.s4),
          _closeButton(primary: false),
        ];
      default: // starting, connecting, joining
        return [
          Row(children: [
            SizedBox(
              width: ErsmSpace.s4,
              height: ErsmSpace.s4,
              child: CircularProgressIndicator(
                  strokeWidth: 2,
                  color: _dark ? ErsmDark.accent : ErsmLight.accent),
            ),
            const SizedBox(width: ErsmSpace.s3),
            Expanded(child: _title('Conectando con IT…', margin: false)),
          ]),
          const SizedBox(height: ErsmSpace.s3),
          _paragraph('No se instala nada en tu equipo.'),
        ];
    }
  }

  Widget _title(String text, {bool margin = true}) => Padding(
        padding: EdgeInsets.only(bottom: margin ? ErsmSpace.s2 : 0),
        child: Text(text,
            style: TextStyle(
                color: _text,
                fontSize: ErsmText.lgSize,
                fontWeight: FontWeight.w600)),
      );

  Widget _paragraph(String text) => Padding(
        padding: const EdgeInsets.only(bottom: ErsmSpace.s2),
        child: Text(text,
            style: TextStyle(
                color: _muted,
                fontSize: ErsmText.smSize,
                height: ErsmText.smHeight)),
      );

  // Session-active notice (attention role, as SessionBanner in components.md).
  Widget _banner(String text) => Container(
        padding: const EdgeInsets.all(ErsmSpace.s3),
        decoration: BoxDecoration(
          color: _dark ? ErsmDark.attention : ErsmLight.attention,
          borderRadius: BorderRadius.circular(ErsmRadius.md),
        ),
        child: Text(text,
            style: TextStyle(
                color: _dark ? ErsmDark.onAttention : ErsmLight.onAttention,
                fontSize: ErsmText.smSize,
                fontWeight: FontWeight.w600)),
      );

  Widget _message(String text) => Container(
        padding: const EdgeInsets.all(ErsmSpace.s3),
        decoration: BoxDecoration(
          color: _dark ? ErsmDark.dangerSurface : ErsmLight.dangerSurface,
          border: Border.all(color: _dark ? ErsmDark.danger : ErsmLight.danger),
          borderRadius: BorderRadius.circular(ErsmRadius.md),
        ),
        child: Text(text,
            style: TextStyle(
                color: _text,
                fontSize: ErsmText.smSize,
                height: ErsmText.smHeight)),
      );

  Widget _closeButton({required bool primary}) {
    final accent = _dark ? ErsmDark.accent : ErsmLight.accent;
    final onAccent = _dark ? ErsmDark.onAccent : ErsmLight.onAccent;
    final style = ButtonStyle(
      minimumSize: const WidgetStatePropertyAll(Size.fromHeight(40)),
      backgroundColor:
          WidgetStatePropertyAll(primary ? accent : Colors.transparent),
      foregroundColor: WidgetStatePropertyAll(primary ? onAccent : _text),
      side: primary
          ? null
          : WidgetStatePropertyAll(BorderSide(
              color: _dark ? ErsmDark.borderInput : ErsmLight.borderInput)),
      shape: WidgetStatePropertyAll(RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ErsmRadius.md))),
    );
    return TextButton(
      style: style,
      onPressed: () async {
        await windowManager.setPreventClose(false);
        await windowManager.close();
      },
      child: const Text('Cerrar', style: TextStyle(fontWeight: FontWeight.w600)),
    );
  }
}
