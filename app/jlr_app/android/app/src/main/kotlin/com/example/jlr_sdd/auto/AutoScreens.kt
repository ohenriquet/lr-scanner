@file:Suppress("DEPRECATION") // setTitle/setHeaderAction: compatíveis com hosts antigos

package com.example.jlr_sdd.auto

import androidx.car.app.CarContext
import androidx.car.app.CarToast
import androidx.car.app.constraints.ConstraintManager
import androidx.car.app.model.Action
import androidx.car.app.model.ActionStrip
import androidx.car.app.model.CarColor
import androidx.car.app.model.CarIcon
import androidx.car.app.model.GridItem
import androidx.car.app.model.GridTemplate
import androidx.car.app.model.ItemList
import androidx.car.app.model.ListTemplate
import androidx.car.app.model.MessageTemplate
import androidx.car.app.model.Pane
import androidx.car.app.model.PaneTemplate
import androidx.car.app.model.Row
import androidx.car.app.model.Template
import androidx.car.app.versioning.CarAppApiLevels
import androidx.core.graphics.drawable.IconCompat
import androidx.lifecycle.LifecycleOwner
import com.example.jlr_sdd.R
import org.json.JSONObject
import kotlin.math.abs

// Regras do host que guiam o desenho destas telas:
// - Atualizar uma tela só é "refresh" (não conta no limite de 5 telas por
//   tarefa) se o título e os títulos das linhas/blocos não mudam. Por isso os
//   títulos são fixos e o que muda (contagens, valores, VIN) fica no texto.
// - Dirigindo, listas e painéis têm poucas linhas (ConstraintManager).

private fun CarContext.icon(res: Int, tint: CarColor? = null): CarIcon =
    CarIcon.Builder(IconCompat.createWithResource(this, res)).apply {
        if (tint != null) setTint(tint)
    }.build()

private fun CarContext.limit(type: Int, fallback: Int): Int =
    if (carAppApiLevel >= CarAppApiLevels.LEVEL_2) {
        runCatching { getCarService(ConstraintManager::class.java).getContentLimit(type) }
            .getOrDefault(fallback)
    } else fallback

private fun CarContext.toast(res: Int) =
    CarToast.makeText(this, getString(res), CarToast.LENGTH_LONG).show()

private fun fmt(v: Double?, unit: String): String {
    if (v == null) return "—"
    val a = abs(v)
    val n = when {
        a >= 100 -> "%.0f".format(v)
        a >= 10 -> "%.1f".format(v)
        else -> "%.2f".format(v)
    }
    return "$n $unit".trim()
}

/** Título traduzido de um grupo de sinais (ids definidos em `auto_bridge.dart`). */
private fun groupTitle(id: String): Int = when (id) {
    "suspension" -> R.string.group_suspension
    "engine" -> R.string.group_engine
    "electrical" -> R.string.group_electrical
    "transmission" -> R.string.group_transmission
    else -> R.string.group_phone
}

private fun groupIcon(id: String): Int = when (id) {
    "suspension" -> R.drawable.ic_auto_suspension
    "engine" -> R.drawable.ic_auto_engine
    "electrical" -> R.drawable.ic_auto_battery
    "transmission" -> R.drawable.ic_auto_gear
    else -> R.drawable.ic_auto_phone
}

private val GROUPS = listOf("suspension", "engine", "electrical", "transmission", "phone")

// ───────────────────────────── Início ─────────────────────────────

/** Tela inicial: 4 blocos fixos (Falhas, Ao vivo, Limpar, Conexão). */
class HomeScreen(carContext: CarContext) : LrScreen(carContext) {
    private var drawn: String? = null

    // Só o que esta tela mostra (os valores ao vivo não entram).
    private fun signature(s: Snap): String {
        val d = s.dtcs
        return listOf(
            AutoBridge.ready, AutoBridge.packError, s.connected, s.connecting, s.reading,
            s.scanning, s.scanDone, s.scanTotal, s.clearing, s.demo, s.error, s.vin,
            d.size, d.count { it.confirmed },
        ).joinToString("|")
    }

    override fun shouldRedraw(): Boolean = signature(snap) != drawn

    override fun onGetTemplate(): Template {
        drawn = signature(snap)
        if (!AutoBridge.ready) {
            return GridTemplate.Builder()
                .setTitle(str(R.string.app_name))
                .setHeaderAction(Action.APP_ICON)
                .setLoading(true)
                .build()
        }
        AutoBridge.packError?.let { err ->
            return MessageTemplate.Builder(str(R.string.auto_pack_error, err))
                .setTitle(str(R.string.app_name))
                .setHeaderAction(Action.APP_ICON)
                .build()
        }
        val s = snap
        val items = ItemList.Builder()
            .addItem(faultsItem(s))
            .addItem(liveItem(s))
            .addItem(clearItem(s))
            .addItem(connectionItem(s))
            .build()
        val b = GridTemplate.Builder()
            .setTitle(str(R.string.app_name))
            .setHeaderAction(Action.APP_ICON)
            .setSingleList(items)
        val strip = ActionStrip.Builder()
        var hasAction = false
        if (s.connected && !s.busy) {
            strip.addAction(
                Action.Builder().setTitle(str(R.string.action_reread))
                    .setOnClickListener { AutoBridge.refresh() }.build()
            )
            hasAction = true
        } else if (!s.connected && !s.connecting) {
            strip.addAction(
                Action.Builder().setTitle(str(R.string.action_demo))
                    .setOnClickListener { AutoBridge.demo() }.build()
            )
            hasAction = true
        }
        if (hasAction) b.setActionStrip(strip.build())
        return b.build()
    }

    private fun faultsItem(s: Snap): GridItem {
        val dtcs = s.dtcs
        val confirmed = dtcs.count { it.confirmed }
        val text = when {
            !s.connected -> str(R.string.txt_connect_first)
            s.reading || s.scanning -> str(R.string.txt_reading_progress, s.scanDone, s.scanTotal)
            dtcs.isEmpty() -> str(R.string.txt_no_faults)
            else -> str(R.string.txt_faults_summary, confirmed, dtcs.size)
        }
        val tint = when {
            !s.connected -> null
            confirmed > 0 -> CarColor.RED
            dtcs.isNotEmpty() -> CarColor.YELLOW
            else -> CarColor.GREEN
        }
        return GridItem.Builder()
            .setTitle(str(R.string.tile_faults))
            .setText(text)
            .setImage(carContext.icon(R.drawable.ic_auto_faults, tint), GridItem.IMAGE_TYPE_LARGE)
            .setOnClickListener {
                if (!snap.connected) carContext.toast(R.string.txt_connect_first)
                else screenManager.push(FaultsScreen(carContext))
            }
            .build()
    }

    private fun liveItem(s: Snap): GridItem = GridItem.Builder()
        .setTitle(str(R.string.tile_live))
        .setText(str(if (s.connected) R.string.txt_live_hint else R.string.txt_connect_first))
        .setImage(carContext.icon(R.drawable.ic_auto_live), GridItem.IMAGE_TYPE_LARGE)
        .setOnClickListener {
            if (!snap.connected) carContext.toast(R.string.txt_connect_first)
            else screenManager.push(LiveGroupsScreen(carContext))
        }
        .build()

    private fun clearItem(s: Snap): GridItem = GridItem.Builder()
        .setTitle(str(R.string.tile_clear))
        .setText(str(if (s.connected) R.string.txt_clear_hint else R.string.txt_connect_first))
        .setImage(
            carContext.icon(R.drawable.ic_auto_clear, if (s.connected) CarColor.RED else null),
            GridItem.IMAGE_TYPE_LARGE
        )
        .setOnClickListener {
            val now = snap
            when {
                !now.connected -> carContext.toast(R.string.txt_connect_first)
                now.busy -> carContext.toast(R.string.toast_wait)
                now.dtcs.isEmpty() -> carContext.toast(R.string.toast_no_faults)
                else -> screenManager.push(ClearScreen(carContext, module = null))
            }
        }
        .build()

    private fun connectionItem(s: Snap): GridItem {
        val b = GridItem.Builder().setTitle(str(R.string.tile_connection))
        if (s.connecting) {
            return b.setText(str(R.string.txt_connecting)).setLoading(true).build()
        }
        val text = when {
            s.connected && s.demo -> str(R.string.txt_demo)
            s.connected -> s.vin?.let { str(R.string.txt_connected_vin, it) }
                ?: str(R.string.txt_connected)
            s.error != null -> s.error!!
            else -> str(R.string.txt_tap_to_connect)
        }
        return b.setText(text)
            .setImage(
                carContext.icon(R.drawable.ic_auto_usb, if (s.connected) CarColor.GREEN else null),
                GridItem.IMAGE_TYPE_LARGE
            )
            .setOnClickListener {
                val now = snap
                when {
                    now.connected -> screenManager.push(DisconnectScreen(carContext))
                    now.connecting -> Unit
                    else -> {
                        carContext.toast(R.string.toast_connecting)
                        AutoBridge.connect()
                    }
                }
            }
            .build()
    }
}

// ───────────────────────────── Falhas ─────────────────────────────

/** Lista das falhas (confirmadas primeiro). "Reler" volta ao início com o progresso. */
class FaultsScreen(carContext: CarContext) : LrScreen(carContext) {
    private var drawn: String? = null

    private fun signature(s: Snap) =
        "${s.connected}|${s.reading || s.scanning}|" + s.dtcs.joinToString(",") { it.module + it.code + it.status }

    override fun shouldRedraw(): Boolean = signature(snap) != drawn

    override fun onGetTemplate(): Template {
        val s = snap
        drawn = signature(s)
        val b = ListTemplate.Builder()
            .setTitle(str(R.string.faults_title))
            .setHeaderAction(Action.BACK)
            .setActionStrip(
                ActionStrip.Builder().addAction(
                    Action.Builder().setTitle(str(R.string.action_reread)).setOnClickListener {
                        AutoBridge.refresh()
                        screenManager.pop() // o progresso aparece no bloco Falhas
                    }.build()
                ).build()
            )
        if (s.reading || s.scanning) return b.setLoading(true).build()

        val dtcs = s.dtcs
        val max = carContext.limit(ConstraintManager.CONTENT_LIMIT_TYPE_LIST, 6)
        val shown = if (dtcs.size > max) dtcs.take(max - 1) else dtcs
        val list = ItemList.Builder().setNoItemsMessage(str(R.string.faults_empty))
        for (d in shown) {
            val status = "%02X".format(d.status)
            list.addItem(
                Row.Builder()
                    .setTitle("${d.code} · ${d.module}")
                    .addText(d.desc.ifBlank { str(R.string.fault_no_desc) })
                    .addText(
                        str(if (d.confirmed) R.string.fault_confirmed else R.string.fault_history, status)
                    )
                    .setImage(
                        carContext.icon(
                            R.drawable.ic_auto_dot,
                            if (d.confirmed) CarColor.RED else CarColor.YELLOW
                        )
                    )
                    .setOnClickListener { screenManager.push(FaultDetailScreen(carContext, d)) }
                    .build()
            )
        }
        if (shown.size < dtcs.size) {
            list.addItem(Row.Builder().setTitle(str(R.string.faults_more, dtcs.size - shown.size)).build())
        }
        return b.setSingleList(list.build()).build()
    }
}

/** Detalhe de uma falha: descrição + help do SDD + "Limpar este módulo". */
class FaultDetailScreen(carContext: CarContext, private val dtc: Dtc) : LrScreen(carContext) {
    private var help: JSONObject? = null

    init {
        AutoBridge.dtcHelp(dtc.module, dtc.code) {
            help = it ?: JSONObject()
            invalidate()
        }
    }

    override fun shouldRedraw() = false

    override fun onGetTemplate(): Template {
        val b = PaneTemplate.Builder(buildPane()).setTitle(dtc.code).setHeaderAction(Action.BACK)
        return b.build()
    }

    private fun buildPane(): Pane {
        val h = help ?: return Pane.Builder().setLoading(true).build()
        val max = carContext.limit(ConstraintManager.CONTENT_LIMIT_TYPE_PANE, 4)
        val rows = mutableListOf<Row>()
        val status = "%02X".format(dtc.status)
        rows += Row.Builder()
            .setTitle(dtc.desc.ifBlank { h.optString("desc").ifBlank { str(R.string.fault_no_desc) } })
            .addText(
                str(
                    R.string.fault_module, dtc.module,
                    str(if (dtc.confirmed) R.string.fault_confirmed else R.string.fault_history, status)
                )
            )
            .build()
        for ((key, title) in listOf(
            "causes" to R.string.help_causes,
            "actions" to R.string.help_actions,
            "symptoms" to R.string.help_symptoms,
            "supp" to R.string.help_supp,
        )) {
            val items = h.optJSONArray(key).strings().filter { it.isNotBlank() }
            if (items.isEmpty() || rows.size >= max) continue
            rows += Row.Builder()
                .setTitle(str(title))
                .addText(items.take(3).joinToString(" · "))
                .build()
        }
        val pane = Pane.Builder()
        rows.forEach { pane.addRow(it) }
        pane.addAction(
            Action.Builder()
                .setTitle(str(R.string.action_clear_module))
                .setBackgroundColor(CarColor.RED)
                .setOnClickListener {
                    val now = snap
                    when {
                        !now.connected -> carContext.toast(R.string.txt_connect_first)
                        now.busy -> carContext.toast(R.string.toast_wait)
                        else -> screenManager.push(ClearScreen(carContext, module = dtc.module))
                    }
                }
                .build()
        )
        return pane.build()
    }
}

// ───────────────────────────── Limpar ─────────────────────────────

/** Confirmação → limpando → resultado. [module] null = todos os módulos. */
class ClearScreen(carContext: CarContext, private val module: String?) : LrScreen(carContext) {
    private enum class Step { CONFIRM, RUNNING, DONE }

    private var step = Step.CONFIRM
    private var result: JSONObject? = null

    override fun shouldRedraw() = false

    override fun onGetTemplate(): Template {
        val title = str(R.string.clear_title)
        return when (step) {
            Step.CONFIRM -> {
                val msg = if (module == null) str(R.string.clear_confirm_all, snap.online)
                else str(R.string.clear_confirm_module, module)
                MessageTemplate.Builder(msg)
                    .setTitle(title)
                    .setHeaderAction(Action.BACK)
                    .setIcon(carContext.icon(R.drawable.ic_auto_clear, CarColor.RED))
                    .addAction(
                        Action.Builder().setTitle(str(R.string.action_clear))
                            .setBackgroundColor(CarColor.RED)
                            .setOnClickListener { run() }.build()
                    )
                    .addAction(
                        Action.Builder().setTitle(str(R.string.action_cancel))
                            .setOnClickListener { screenManager.pop() }.build()
                    )
                    .build()
            }
            Step.RUNNING -> MessageTemplate.Builder(str(R.string.clear_running))
                .setTitle(title)
                .setLoading(true)
                .build()
            Step.DONE -> {
                val r = result
                val err = r?.optStringOrNull("error")
                val msg = when {
                    r == null -> str(R.string.clear_error, "—")
                    err != null -> str(R.string.clear_error, err)
                    else -> str(
                        R.string.clear_result,
                        r.optJSONArray("cleared")?.length() ?: 0,
                        r.optJSONArray("refused")?.length() ?: 0,
                        r.optJSONArray("noResponse")?.length() ?: 0,
                        r.optInt("remaining"),
                    )
                }
                MessageTemplate.Builder(msg)
                    .setTitle(title)
                    .setIcon(
                        carContext.icon(
                            R.drawable.ic_auto_faults,
                            if (err == null && r != null) CarColor.GREEN else CarColor.RED
                        )
                    )
                    .addAction(
                        Action.Builder().setTitle(str(R.string.action_ok))
                            .setOnClickListener { screenManager.popToRoot() }.build()
                    )
                    .build()
            }
        }
    }

    private fun run() {
        step = Step.RUNNING
        invalidate()
        AutoBridge.clearDtcs(module) {
            result = it
            step = Step.DONE
            invalidate()
        }
    }
}

/** Confirma antes de desconectar (um toque acidental dirigindo não derruba a sessão). */
class DisconnectScreen(carContext: CarContext) : LrScreen(carContext) {
    override fun shouldRedraw() = false

    override fun onGetTemplate(): Template =
        MessageTemplate.Builder(str(R.string.disconnect_confirm))
            .setTitle(str(R.string.tile_connection))
            .setHeaderAction(Action.BACK)
            .addAction(
                Action.Builder().setTitle(str(R.string.action_disconnect))
                    .setOnClickListener {
                        AutoBridge.disconnect()
                        screenManager.pop()
                    }.build()
            )
            .addAction(
                Action.Builder().setTitle(str(R.string.action_cancel))
                    .setOnClickListener { screenManager.pop() }.build()
            )
            .build()
}

// ───────────────────────────── Ao vivo ─────────────────────────────

/** Grupos de sinais (suspensão, motor, elétrica, câmbio, observados no celular). */
class LiveGroupsScreen(carContext: CarContext) : LrScreen(carContext) {
    private var drawn: String? = null
    private fun signature(s: Snap) = GROUPS.joinToString(",") { "$it=${s.groupKeys(it).size}" }
    override fun shouldRedraw() = signature(snap) != drawn

    override fun onGetTemplate(): Template {
        val s = snap
        drawn = signature(s)
        val list = ItemList.Builder().setNoItemsMessage(str(R.string.live_empty))
        for (id in GROUPS) {
            val n = s.groupKeys(id).size
            if (n == 0) continue
            list.addItem(
                Row.Builder()
                    .setTitle(str(groupTitle(id)))
                    .addText(str(R.string.group_count, n))
                    .setImage(carContext.icon(groupIcon(id)))
                    .setBrowsable(true)
                    .setOnClickListener { screenManager.push(LiveGroupScreen(carContext, id)) }
                    .build()
            )
        }
        return ListTemplate.Builder()
            .setTitle(str(R.string.live_title))
            .setHeaderAction(Action.BACK)
            .setSingleList(list.build())
            .build()
    }
}

/** Sinais de um grupo: valor atual, mín/máx e sparkline. Atualiza a ~1 Hz. */
class LiveGroupScreen(carContext: CarContext, private val group: String) :
    LrScreen(carContext, minRefreshMs = 1000) {

    override fun onStart(owner: LifecycleOwner) {
        super.onStart(owner)
        AutoBridge.watch(group)
    }

    override fun onDestroy(owner: LifecycleOwner) {
        AutoBridge.unwatch()
    }

    override fun onGetTemplate(): Template {
        val s = snap
        val b = ListTemplate.Builder()
            .setTitle(str(groupTitle(group)))
            .setHeaderAction(Action.BACK)
        val keys = s.groupKeys(group)
        val live = s.live
        // Só desenha quando todos os sinais chegaram: as linhas não mudam depois.
        if (keys.isEmpty() || keys.any { it !in live }) return b.setLoading(true).build()

        val max = carContext.limit(ConstraintManager.CONTENT_LIMIT_TYPE_LIST, 6)
        val list = ItemList.Builder()
        for (key in keys.take(max)) {
            val sig = live.getValue(key)
            list.addItem(
                Row.Builder()
                    .setTitle(sig.name)
                    .addText(sig.display)
                    .addText(str(R.string.live_minmax, fmt(sig.min, sig.unit), fmt(sig.max, sig.unit)))
                    .setImage(ChartRenderer.sparkline(sig.history, carContext.isDarkMode), Row.IMAGE_TYPE_LARGE)
                    .setOnClickListener { screenManager.push(ChartScreen(carContext, group, key)) }
                    .build()
            )
        }
        return b.setSingleList(list.build()).build()
    }
}

/** Gráfico grande de um sinal (últimos 60 s) + agora/mín/máx. */
class ChartScreen(carContext: CarContext, private val group: String, private val key: String) :
    LrScreen(carContext, minRefreshMs = 1000) {

    override fun onStart(owner: LifecycleOwner) {
        super.onStart(owner)
        AutoBridge.watch(group) // ao voltar ao app com esta tela no topo
    }

    override fun onGetTemplate(): Template {
        val sig = snap.live[key]
        val title = sig?.name ?: str(R.string.live_title)
        val pane = Pane.Builder()
        if (sig == null) {
            pane.setLoading(true)
        } else {
            pane.addRow(Row.Builder().setTitle(str(R.string.chart_now)).addText(sig.display).build())
            pane.addRow(Row.Builder().setTitle(str(R.string.chart_min)).addText(fmt(sig.min, sig.unit)).build())
            pane.addRow(Row.Builder().setTitle(str(R.string.chart_max)).addText(fmt(sig.max, sig.unit)).build())
            if (carContext.carAppApiLevel >= CarAppApiLevels.LEVEL_4) {
                pane.setImage(
                    ChartRenderer.chart(sig.history, sig.unit, carContext.isDarkMode, str(R.string.chart_now_axis))
                )
            }
        }
        return PaneTemplate.Builder(pane.build())
            .setTitle(title)
            .setHeaderAction(Action.BACK)
            .build()
    }
}
