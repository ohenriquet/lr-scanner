package com.example.jlr_sdd

import io.flutter.embedding.android.FlutterActivity

/**
 * Tela do celular. Usa a engine compartilhada (ver [LrScannerApp]) em vez de
 * criar uma própria, para partilhar o estado com o Android Auto. A conexão USB
 * com o Mongoose fica em [MongooseUsb].
 */
class MainActivity : FlutterActivity() {
    override fun getCachedEngineId(): String {
        LrScannerApp.engine(this) // garante que existe
        return LrScannerApp.ENGINE_ID
    }
}
