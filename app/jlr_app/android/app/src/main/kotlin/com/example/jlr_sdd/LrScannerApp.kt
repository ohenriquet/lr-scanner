package com.example.jlr_sdd

import android.app.Application
import android.content.Context
import com.example.jlr_sdd.auto.AutoBridge
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.FlutterEngineCache
import io.flutter.embedding.engine.dart.DartExecutor

/**
 * Cria UMA engine do Flutter por processo e a guarda no cache. A tela do
 * celular (MainActivity) e a tela do carro (Android Auto) usam a mesma engine:
 * o mesmo estado do carro e a mesma sessão com o Mongoose, mesmo quando o app
 * só está aberto no carro.
 */
class LrScannerApp : Application() {
    override fun onCreate() {
        super.onCreate()
        engine(this)
    }

    companion object {
        const val ENGINE_ID = "main"

        /** A engine compartilhada, criada (e com o Dart rodando) na primeira chamada. */
        fun engine(context: Context): FlutterEngine {
            FlutterEngineCache.getInstance().get(ENGINE_ID)?.let { return it }
            val app = context.applicationContext
            val engine = FlutterEngine(app)
            val messenger = engine.dartExecutor.binaryMessenger
            // Os canais existem antes do Dart rodar: nenhuma chamada se perde.
            MongooseUsb.attach(app, messenger)
            AutoBridge.attach(messenger)
            engine.dartExecutor.executeDartEntrypoint(
                DartExecutor.DartEntrypoint.createDefault()
            )
            FlutterEngineCache.getInstance().put(ENGINE_ID, engine)
            return engine
        }
    }
}
