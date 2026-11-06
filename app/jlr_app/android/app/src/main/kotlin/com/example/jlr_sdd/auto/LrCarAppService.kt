package com.example.jlr_sdd.auto

import android.content.Intent
import android.content.pm.ApplicationInfo
import android.os.Handler
import android.os.Looper
import androidx.car.app.CarAppService
import androidx.car.app.CarContext
import androidx.car.app.Screen
import androidx.car.app.Session
import androidx.car.app.validation.HostValidator
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import com.example.jlr_sdd.LrScannerApp
import com.example.jlr_sdd.MongooseUsb

/**
 * Entrada do Android Auto. As telas são templates (Car App Library); a lógica
 * fica no Dart, na engine compartilhada (ver [LrScannerApp] e [AutoBridge]).
 */
class LrCarAppService : CarAppService() {
    override fun onCreate() {
        super.onCreate()
        LrScannerApp.engine(this) // a engine (e o Dart) já sobe com o processo
    }

    override fun createHostValidator(): HostValidator =
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) {
            HostValidator.ALLOW_ALL_HOSTS_VALIDATOR
        } else {
            // Release: só hosts conhecidos (Android Auto e afins).
            HostValidator.Builder(applicationContext)
                .addAllowedHosts(androidx.car.app.R.array.hosts_allowlist_sample)
                .build()
        }

    override fun onCreateSession(): Session = LrSession()
}

class LrSession : Session(), DefaultLifecycleObserver {
    private var triedAutoConnect = false

    init {
        lifecycle.addObserver(this)
    }

    override fun onCreateScreen(intent: Intent): Screen = HomeScreen(carContext)

    override fun onStart(owner: LifecycleOwner) {
        AutoBridge.setActive(true) { snap ->
            // Abriu no carro com o Mongoose plugado e já autorizado: conecta sozinho.
            if (!triedAutoConnect && !snap.connected && !snap.connecting &&
                MongooseUsb.isAttachedWithPermission()
            ) {
                triedAutoConnect = true
                AutoBridge.connect()
            }
        }
    }

    override fun onStop(owner: LifecycleOwner) {
        AutoBridge.setActive(false)
    }
}

/**
 * Base das telas: redesenha quando o estado do Dart muda, no máximo a cada
 * [minRefreshMs] (o host limita atualizações; valores ao vivo a ~1 Hz bastam).
 */
abstract class LrScreen(carContext: CarContext, private val minRefreshMs: Long = 400) :
    Screen(carContext), DefaultLifecycleObserver {

    private val handler = Handler(Looper.getMainLooper())
    private var lastInvalidate = 0L
    private var scheduled = false

    private val onState: () -> Unit = {
        if (!scheduled) {
            val wait = (lastInvalidate + minRefreshMs - System.currentTimeMillis()).coerceAtLeast(0)
            scheduled = true
            handler.postDelayed({
                scheduled = false
                lastInvalidate = System.currentTimeMillis()
                if (shouldRedraw()) invalidate()
            }, wait)
        }
    }

    init {
        lifecycle.addObserver(this)
    }

    /** Decide se o novo estado muda esta tela (evita refresh inútil). */
    open fun shouldRedraw(): Boolean = true

    override fun onStart(owner: LifecycleOwner) {
        AutoBridge.addListener(onState)
    }

    override fun onStop(owner: LifecycleOwner) {
        AutoBridge.removeListener(onState)
        handler.removeCallbacksAndMessages(null)
        scheduled = false
    }

    protected val snap get() = AutoBridge.snap
    protected fun str(id: Int, vararg args: Any): String = carContext.getString(id, *args)
}
