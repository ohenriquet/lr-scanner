package com.example.jlr_sdd.auto

import android.app.Application
import android.os.Looper
import androidx.car.app.OnDoneCallback
import androidx.car.app.model.GridItem
import androidx.car.app.model.GridTemplate
import androidx.car.app.model.ListTemplate
import androidx.car.app.model.MessageTemplate
import androidx.car.app.model.PaneTemplate
import androidx.car.app.model.Row
import androidx.car.app.testing.TestCarContext
import androidx.car.app.versioning.CarAppApiLevels
import androidx.test.core.app.ApplicationProvider
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * Monta cada tela do carro com estados típicos. Os builders da Car App Library
 * validam as regras dos templates e lançam exceção se algo for proibido — o que,
 * no carro, derrubaria o app. Sem engine do Flutter (Application simples).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = Application::class)
class AutoScreensTest {
    private lateinit var ctx: TestCarContext

    @Before
    fun setUp() {
        ctx = TestCarContext.createCarContext(ApplicationProvider.getApplicationContext())
        // No carro o host define o nível de API no handshake; aqui, por reflexão.
        androidx.car.app.CarContext::class.java.getDeclaredField("mCarAppApiLevel").apply {
            isAccessible = true
            setInt(ctx, CarAppApiLevels.getLatest())
        }
    }

    private fun dtc(module: String, code: String, status: Int) = JSONObject()
        .put("module", module).put("code", code).put("desc", "Descrição de $code")
        .put("status", status).put("confirmed", status and 0x08 != 0)

    private fun signal(key: String, name: String, unit: String, n: Int) = JSONObject()
        .put("key", key).put("module", key.substringBefore('|')).put("did", key.substringAfter('|'))
        .put("name", name).put("unit", unit).put("value", 1.0).put("display", "1.00 $unit")
        .put("min", 0.9).put("max", 1.1)
        .put("history", JSONArray((0 until n).map { 1.0 + 0.1 * Math.sin(it / 5.0) }))

    private fun state(
        connected: Boolean = true,
        connecting: Boolean = false,
        scanning: Boolean = false,
        dtcs: Int = 4,
    ): String {
        val list = JSONArray()
        repeat(dtcs) { i -> list.put(dtc(if (i % 2 == 0) "RLM" else "PCM", "C1A1$i-08", if (i < 2) 0x2F else 0x24)) }
        val keys = listOf("RLM|0x3B03", "RLM|0x3B04", "RLM|0x3B05", "RLM|0x3B06", "RLM|0x3B10")
        return JSONObject()
            .put("connected", connected).put("connecting", connecting).put("reading", false)
            .put("demo", false).put("clearing", false).put("error", JSONObject.NULL)
            .put("vin", if (connected) "SALLAAAF4CA606217" else JSONObject.NULL)
            .put("online", 10).put("modules", 18)
            .put("scanning", scanning).put("scanDone", 4).put("scanTotal", 12)
            .put("dtcs", list)
            .put("groups", JSONObject()
                .put("suspension", JSONArray(keys))
                .put("engine", JSONArray(listOf("PCM|0x0305")))
                .put("electrical", JSONArray())
                .put("transmission", JSONArray())
                .put("phone", JSONArray(listOf("RLM|0x3B03"))))
            .put("live", JSONArray(keys.map { signal(it, "Height $it", "m", 120) } +
                signal("PCM|0x0305", "Engine coolant temperature", "°C", 1)))
            .put("lastClear", JSONObject.NULL)
            .toString()
    }

    private fun idle() = shadowOf(Looper.getMainLooper()).idle()

    @Test
    fun home_loading_until_dart_ready() {
        AutoBridge.setForTest(state(), isReady = false)
        val t = HomeScreen(ctx).onGetTemplate() as GridTemplate
        assertTrue(t.isLoading)
    }

    @Test
    fun home_shows_pack_error() {
        AutoBridge.setForTest(state(), error = "pack.json ausente")
        assertTrue(HomeScreen(ctx).onGetTemplate() is MessageTemplate)
    }

    @Test
    fun home_has_four_fixed_tiles_in_every_state() {
        for (s in listOf(
            state(connected = false, dtcs = 0),
            state(connected = false, connecting = true, dtcs = 0),
            state(scanning = true),
            state(),
        )) {
            AutoBridge.setForTest(s)
            val t = HomeScreen(ctx).onGetTemplate() as GridTemplate
            val titles = t.singleList!!.items.map { (it as GridItem).title.toString() }
            assertEquals(4, titles.size)
        }
        // Os títulos não mudam entre estados (regra de "refresh" do host).
        AutoBridge.setForTest(state(connected = false, connecting = true, dtcs = 0))
        val a = (HomeScreen(ctx).onGetTemplate() as GridTemplate).singleList!!.items.map { (it as GridItem).title.toString() }
        AutoBridge.setForTest(state())
        val b = (HomeScreen(ctx).onGetTemplate() as GridTemplate).singleList!!.items.map { (it as GridItem).title.toString() }
        assertEquals(a, b)
    }

    @Test
    fun faults_list_respects_row_limit_and_loading() {
        AutoBridge.setForTest(state(dtcs = 15))
        val t = FaultsScreen(ctx).onGetTemplate() as ListTemplate
        val rows = t.singleList!!.items
        assertTrue(rows.size in 1..15)
        assertTrue((rows.first() as Row).title.toString().startsWith("C1A10-08"))

        AutoBridge.setForTest(state(scanning = true))
        assertTrue((FaultsScreen(ctx).onGetTemplate() as ListTemplate).isLoading)

        AutoBridge.setForTest(state(dtcs = 0))
        assertEquals(0, (FaultsScreen(ctx).onGetTemplate() as ListTemplate).singleList!!.items.size)
    }

    @Test
    fun fault_detail_builds_after_help() {
        AutoBridge.setForTest(state())
        val d = Dtc("RLM", "C1A13-08", "Front left height sensor", 0x2F, true)
        val screen = FaultDetailScreen(ctx, d) // sem engine: o help chega vazio na hora
        val t = screen.onGetTemplate() as PaneTemplate
        assertTrue(t.pane.rows.isNotEmpty())
        assertEquals(1, t.pane.actions.size)
    }

    @Test
    fun clear_confirm_then_result() {
        AutoBridge.setForTest(state())
        val screen = ClearScreen(ctx, module = null)
        val confirm = screen.onGetTemplate() as MessageTemplate
        assertEquals(2, confirm.actions.size)
        // "Limpar": sem engine o Dart responde null → tela de resultado (erro).
        confirm.actions[0].onClickDelegate!!.sendClick(object : OnDoneCallback {})
        idle()
        val done = screen.onGetTemplate() as MessageTemplate
        assertEquals(1, done.actions.size)

        assertTrue(ClearScreen(ctx, module = "RLM").onGetTemplate() is MessageTemplate)
        assertTrue(DisconnectScreen(ctx).onGetTemplate() is MessageTemplate)
    }

    @Test
    fun live_groups_hide_empty_groups() {
        AutoBridge.setForTest(state())
        val t = LiveGroupsScreen(ctx).onGetTemplate() as ListTemplate
        // suspensão, motor e "observados no celular" (elétrica e câmbio vazios)
        assertEquals(3, t.singleList!!.items.size)
    }

    @Test
    fun live_group_rows_have_sparklines() {
        AutoBridge.setForTest(state())
        val t = LiveGroupScreen(ctx, "suspension").onGetTemplate() as ListTemplate
        val rows = t.singleList!!.items.map { it as Row }
        assertEquals(5, rows.size)
        rows.forEach { assertNotNull(it.image) }
        // Sinal com só uma amostra (sparkline vazia) também monta.
        assertEquals(1, (LiveGroupScreen(ctx, "engine").onGetTemplate() as ListTemplate).singleList!!.items.size)
        // Grupo sem dados ainda → carregando.
        assertTrue((LiveGroupScreen(ctx, "transmission").onGetTemplate() as ListTemplate).isLoading)
    }

    @Test
    fun chart_pane_has_image_and_three_rows() {
        AutoBridge.setForTest(state())
        val t = ChartScreen(ctx, "suspension", "RLM|0x3B03").onGetTemplate() as PaneTemplate
        assertEquals(3, t.pane.rows.size)
        assertNotNull(t.pane.image)
        assertTrue((ChartScreen(ctx, "suspension", "X|0x0").onGetTemplate() as PaneTemplate).pane.isLoading)
    }
}
