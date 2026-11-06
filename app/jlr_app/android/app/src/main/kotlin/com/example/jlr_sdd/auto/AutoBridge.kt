package com.example.jlr_sdd.auto

import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel
import org.json.JSONArray
import org.json.JSONObject

/**
 * Lado Kotlin do canal `lr/auto` (o lado Dart está em `lib/auto_bridge.dart`).
 *
 * O Dart é o cérebro (pacote, sessão com o Mongoose, decodificação); aqui só
 * guardamos o último estado ([snap]) e avisamos as telas do carro. Comandos
 * enviados antes do Dart terminar de carregar ficam numa fila até o `ready`.
 * Tudo roda na main thread (MethodChannel e Car App Library usam a mesma).
 */
object AutoBridge {
    private const val CHANNEL = "lr/auto"

    private var channel: MethodChannel? = null
    private val queue = mutableListOf<() -> Unit>()
    private val listeners = linkedSetOf<() -> Unit>()

    /** O Dart terminou de carregar (com sucesso ou não). */
    var ready = false
        private set

    /** Erro ao carregar o pacote do veículo (sem `pack.json`, por exemplo). */
    var packError: String? = null
        private set

    /** Último estado recebido do Dart. */
    var snap = Snap(JSONObject())
        private set

    fun attach(messenger: BinaryMessenger) {
        channel = MethodChannel(messenger, CHANNEL).also { ch ->
            ch.setMethodCallHandler { call, result ->
                when (call.method) {
                    "ready" -> {
                        val args = call.arguments as? Map<*, *>
                        packError =
                            if (args?.get("ok") == true) null else "${args?.get("error")}"
                        ready = true
                        val pending = queue.toList()
                        queue.clear()
                        pending.forEach { it() }
                        notifyListeners()
                        result.success(null)
                    }
                    "state" -> {
                        update(call.arguments as? String)
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
        }
    }

    /** Testes: simula o Dart pronto com um estado, sem engine. */
    @androidx.annotation.VisibleForTesting
    fun setForTest(json: String, isReady: Boolean = true, error: String? = null) {
        channel = null
        ready = isReady
        packError = error
        snap = Snap(JSONObject(json))
        queue.clear()
        listeners.clear()
    }

    fun addListener(l: () -> Unit) {
        listeners.add(l)
    }

    fun removeListener(l: () -> Unit) {
        listeners.remove(l)
    }

    private fun notifyListeners() = listeners.toList().forEach { it() }

    private fun update(json: String?) {
        if (json == null) return
        snap = try {
            Snap(JSONObject(json))
        } catch (_: Exception) {
            return
        }
        notifyListeners()
    }

    /** Chama um método no Dart; [onResult] recebe o retorno (ou null em erro). */
    fun call(method: String, args: Map<String, Any?>? = null, onResult: ((Any?) -> Unit)? = null) {
        val run: () -> Unit = {
            val ch = channel
            if (ch == null) {
                onResult?.invoke(null)
            } else {
                ch.invokeMethod(method, args, object : MethodChannel.Result {
                    override fun success(result: Any?) {
                        onResult?.invoke(result)
                    }

                    override fun error(code: String, msg: String?, details: Any?) {
                        onResult?.invoke(null)
                    }

                    override fun notImplemented() {
                        onResult?.invoke(null)
                    }
                })
            }
        }
        if (ready) run() else queue.add(run)
    }

    /** A tela do carro abriu/fechou: o Dart só manda estado enquanto ativa. */
    fun setActive(active: Boolean, onSnap: ((Snap) -> Unit)? = null) {
        call("active", mapOf("active" to active)) { r ->
            update(r as? String)
            onSnap?.invoke(snap)
        }
    }

    fun connect() = call("connect")
    fun disconnect() = call("disconnect")
    fun refresh() = call("refresh")
    fun demo() = call("demo")
    fun watch(group: String) = call("watch", mapOf("group" to group))
    fun unwatch() = call("unwatch")

    fun clearDtcs(module: String?, onDone: (JSONObject?) -> Unit) =
        call("clearDtcs", mapOf("module" to module)) { r ->
            onDone((r as? String)?.let { runCatching { JSONObject(it) }.getOrNull() })
        }

    fun dtcHelp(module: String, code: String, onDone: (JSONObject?) -> Unit) =
        call("dtcHelp", mapOf("module" to module, "code" to code)) { r ->
            onDone((r as? String)?.let { runCatching { JSONObject(it) }.getOrNull() })
        }
}

/** Leitura tipada do snapshot JSON montado em `autoSnapshot()` (Dart). */
class Snap(private val j: JSONObject) {
    val connected get() = j.optBoolean("connected")
    val connecting get() = j.optBoolean("connecting")
    val reading get() = j.optBoolean("reading")
    val demo get() = j.optBoolean("demo")
    val clearing get() = j.optBoolean("clearing")
    val error: String? get() = j.optStringOrNull("error")
    val vin: String? get() = j.optStringOrNull("vin")
    val online get() = j.optInt("online")
    val modules get() = j.optInt("modules")
    val scanning get() = j.optBoolean("scanning")
    val scanDone get() = j.optInt("scanDone")
    val scanTotal get() = j.optInt("scanTotal")
    val busy get() = connecting || reading || scanning || clearing

    val dtcs: List<Dtc>
        get() = j.optJSONArray("dtcs").objects().map {
            Dtc(
                module = it.optString("module"),
                code = it.optString("code"),
                desc = it.optString("desc"),
                status = it.optInt("status"),
                confirmed = it.optBoolean("confirmed"),
            )
        }

    /** Chaves `MODULO|DID` de cada grupo (`suspension`, `engine`, ..., `phone`). */
    fun groupKeys(id: String): List<String> =
        j.optJSONObject("groups")?.optJSONArray(id).strings()

    val live: Map<String, Signal>
        get() = j.optJSONArray("live").objects().associate {
            val key = it.optString("key")
            key to Signal(
                key = key,
                module = it.optString("module"),
                name = it.optString("name"),
                unit = it.optString("unit"),
                display = it.optString("display", "…"),
                min = it.optDoubleOrNull("min"),
                max = it.optDoubleOrNull("max"),
                history = it.optJSONArray("history").doubles(),
            )
        }
}

data class Dtc(
    val module: String,
    val code: String,
    val desc: String,
    val status: Int,
    val confirmed: Boolean,
)

data class Signal(
    val key: String,
    val module: String,
    val name: String,
    val unit: String,
    val display: String,
    val min: Double?,
    val max: Double?,
    val history: List<Double>,
)

internal fun JSONObject.optStringOrNull(k: String): String? =
    if (isNull(k)) null else optString(k).ifEmpty { null }

internal fun JSONObject.optDoubleOrNull(k: String): Double? =
    if (isNull(k)) null else optDouble(k).takeUnless { it.isNaN() }

internal fun JSONArray?.objects(): List<JSONObject> =
    if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }

internal fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).map { optString(it) }

internal fun JSONArray?.doubles(): List<Double> =
    if (this == null) emptyList()
    else (0 until length()).map { optDouble(it) }.filter { !it.isNaN() }
