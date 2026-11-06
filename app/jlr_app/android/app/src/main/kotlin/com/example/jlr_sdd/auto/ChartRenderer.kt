package com.example.jlr_sdd.auto

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.DashPathEffect
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Shader
import android.graphics.Typeface
import androidx.car.app.model.CarIcon
import androidx.core.graphics.drawable.IconCompat
import kotlin.math.abs
import kotlin.math.max

/**
 * Desenha os gráficos da tela do carro em Bitmap (os templates do Android Auto
 * não têm gráfico nativo; a imagem vai num CarIcon).
 */
object ChartRenderer {
    private val ACCENT = Color.rgb(0x2D, 0xD4, 0xBF)

    /** Sparkline compacta para a imagem de uma linha da lista. */
    fun sparkline(values: List<Double>, dark: Boolean): CarIcon {
        val w = 192
        val h = 112
        val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val c = Canvas(bmp)
        val pad = 8f
        if (values.size < 2) {
            val p = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = if (dark) Color.argb(90, 255, 255, 255) else Color.argb(90, 0, 0, 0)
                strokeWidth = 4f
                pathEffect = DashPathEffect(floatArrayOf(10f, 10f), 0f)
            }
            c.drawLine(pad, h / 2f, w - pad, h / 2f, p)
            return icon(bmp)
        }
        drawSeries(c, values, pad, pad, w - pad, h - pad, stroke = 5f, dot = 7f)
        return icon(bmp)
    }

    /**
     * Gráfico grande dos últimos ~60 s, com grade, rótulos de mín/máx no eixo
     * vertical e "−60 s … agora" no horizontal.
     */
    fun chart(values: List<Double>, unit: String, dark: Boolean, labelNow: String): CarIcon {
        val w = 960
        val h = 720
        val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val c = Canvas(bmp)
        val ink = if (dark) Color.argb(200, 238, 243, 247) else Color.argb(200, 21, 32, 42)
        val faint = if (dark) Color.argb(50, 255, 255, 255) else Color.argb(40, 0, 0, 0)
        val text = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = ink
            textSize = 34f
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
        }
        val grid = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = faint
            strokeWidth = 2f
        }
        val left = 150f
        val right = w - 30f
        val top = 40f
        val bottom = h - 80f

        if (values.size < 2) {
            c.drawLine(left, (top + bottom) / 2, right, (top + bottom) / 2, grid)
            text.textAlign = Paint.Align.CENTER
            c.drawText("…", (left + right) / 2, (top + bottom) / 2 - 20f, text)
            return icon(bmp)
        }

        val (lo, hi) = range(values)
        // 4 faixas horizontais com rótulo do valor.
        text.textAlign = Paint.Align.RIGHT
        for (i in 0..4) {
            val y = top + (bottom - top) * i / 4f
            c.drawLine(left, y, right, y, grid)
            val v = hi - (hi - lo) * i / 4.0
            c.drawText(fmt(v), left - 16f, y + 12f, text)
        }
        // Eixo do tempo.
        text.textAlign = Paint.Align.LEFT
        c.drawText("−60 s", left, h - 28f, text)
        text.textAlign = Paint.Align.RIGHT
        c.drawText(labelNow, right, h - 28f, text)
        if (unit.isNotBlank()) {
            text.textAlign = Paint.Align.CENTER
            c.drawText(unit, (left + right) / 2, h - 28f, text)
        }
        drawSeries(c, values, left, top, right, bottom, stroke = 7f, dot = 12f, lo = lo, hi = hi,
            slots = HISTORY_SLOTS)
        return icon(bmp)
    }

    /** Amostras que cabem na largura do gráfico grande (60 s a ~2 Hz). */
    private const val HISTORY_SLOTS = 120

    private fun drawSeries(
        c: Canvas, values: List<Double>,
        left: Float, top: Float, right: Float, bottom: Float,
        stroke: Float, dot: Float,
        lo: Double = range(values).first, hi: Double = range(values).second,
        slots: Int = values.size,
    ) {
        val n = max(slots, values.size)
        val start = n - values.size // série alinhada à direita ("agora")
        val dx = (right - left) / (n - 1).coerceAtLeast(1)
        fun x(i: Int) = left + (start + i) * dx
        fun y(v: Double) = (bottom - (v - lo) / (hi - lo) * (bottom - top)).toFloat()

        val line = Path()
        values.forEachIndexed { i, v -> if (i == 0) line.moveTo(x(i), y(v)) else line.lineTo(x(i), y(v)) }
        val fill = Path(line).apply {
            lineTo(x(values.size - 1), bottom)
            lineTo(x(0), bottom)
            close()
        }
        c.drawPath(fill, Paint(Paint.ANTI_ALIAS_FLAG).apply {
            shader = LinearGradient(
                0f, top, 0f, bottom,
                Color.argb(110, 0x2D, 0xD4, 0xBF), Color.argb(0, 0x2D, 0xD4, 0xBF),
                Shader.TileMode.CLAMP
            )
        })
        c.drawPath(line, Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = ACCENT
            style = Paint.Style.STROKE
            strokeWidth = stroke
            strokeJoin = Paint.Join.ROUND
            strokeCap = Paint.Cap.ROUND
        })
        val last = values.size - 1
        c.drawCircle(x(last), y(values[last]), dot, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = ACCENT })
    }

    /** Faixa com folga; um sinal constante ganha uma faixa mínima. */
    private fun range(values: List<Double>): Pair<Double, Double> {
        var lo = values.min()
        var hi = values.max()
        val span = hi - lo
        val minSpan = max(abs(hi) * 0.02, 0.01)
        if (span < minSpan) {
            val mid = (hi + lo) / 2
            lo = mid - minSpan / 2
            hi = mid + minSpan / 2
        } else {
            lo -= span * 0.08
            hi += span * 0.08
        }
        return lo to hi
    }

    private fun fmt(v: Double): String {
        val a = abs(v)
        return when {
            a >= 100 -> "%.0f".format(v)
            a >= 10 -> "%.1f".format(v)
            else -> "%.2f".format(v)
        }
    }

    private fun icon(bmp: Bitmap) = CarIcon.Builder(IconCompat.createWithBitmap(bmp)).build()
}
