package com.example.jlr_sdd

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbManager
import android.os.Build
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel

/**
 * Abre o Mongoose Pro JLR por USB-OTG e devolve o file descriptor do
 * UsbManager para o núcleo nativo (Rust) usar via nusb. EXPERIMENTAL —
 * precisa de validação no dispositivo.
 *
 * Usa o contexto do Application (não de uma Activity), para funcionar também
 * quando a conexão é pedida pela tela do carro (Android Auto).
 *
 * Canal `jlr/usb`: `openMongoose` -> fd (>= 0) ou erro (-1 não encontrado,
 * -2/-3 falha ao abrir, -4 permissão negada, -6 pedido substituído); `close`.
 */
object MongooseUsb {
    private const val CHANNEL = "jlr/usb"

    // Mongoose Pro JLR (Drew Technologies): VID 0x18E1, PID 0x0104.
    private const val VID = 0x18E1
    private const val PID = 0x0104

    private lateinit var app: Context
    private var pendingResult: MethodChannel.Result? = null

    // Mantém a conexão viva; o fd é duplicado pelo lado nativo.
    private var openConnection: UsbDeviceConnection? = null

    private val actionPermission get() = "${app.packageName}.USB_PERMISSION"

    fun attach(context: Context, messenger: BinaryMessenger) {
        app = context.applicationContext
        MethodChannel(messenger, CHANNEL).setMethodCallHandler { call, result ->
            when (call.method) {
                "openMongoose" -> openMongoose(result)
                "close" -> {
                    closeConnection()
                    result.success(true)
                }
                else -> result.notImplemented()
            }
        }
    }

    /** O Mongoose está plugado e o app já tem permissão (conecta sem perguntar). */
    fun isAttachedWithPermission(): Boolean {
        if (!::app.isInitialized) return false
        val device = findDevice() ?: return false
        return usbManager().hasPermission(device)
    }

    private fun usbManager(): UsbManager =
        app.getSystemService(Context.USB_SERVICE) as UsbManager

    private fun findDevice(): UsbDevice? =
        usbManager().deviceList.values.firstOrNull {
            it.vendorId == VID && it.productId == PID
        }

    private fun openMongoose(result: MethodChannel.Result) {
        val mgr = usbManager()
        val device = findDevice()
        if (device == null) {
            result.success(-1) // não encontrado
            return
        }
        if (mgr.hasPermission(device)) {
            result.success(openFd(mgr, device))
            return
        }
        // Um pedido anterior ainda esperando a permissão: responde e substitui.
        pendingResult?.success(-6)
        pendingResult = result
        val flags =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE
            else 0
        val pi = PendingIntent.getBroadcast(
            app, 0, Intent(actionPermission).setPackage(app.packageName), flags
        )
        val filter = IntentFilter(actionPermission)
        try {
            app.unregisterReceiver(permissionReceiver)
        } catch (_: Exception) {
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            app.registerReceiver(permissionReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            app.registerReceiver(permissionReceiver, filter)
        }
        mgr.requestPermission(device, pi)
    }

    private fun closeConnection() {
        try {
            openConnection?.close()
        } catch (_: Exception) {
        }
        openConnection = null
    }

    private fun openFd(mgr: UsbManager, device: UsbDevice): Int {
        return try {
            // Reconexão: a sessão nativa antiga já foi trocada; solta a conexão velha.
            closeConnection()
            val conn = mgr.openDevice(device) ?: return -2
            openConnection = conn
            conn.fileDescriptor // >= 0; o lado nativo duplica
        } catch (e: Exception) {
            -3
        }
    }

    private val permissionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action != actionPermission) return
            try {
                app.unregisterReceiver(this)
            } catch (_: Exception) {
            }
            val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
            val device: UsbDevice? =
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
                    intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
                else
                    @Suppress("DEPRECATION")
                    intent.getParcelableExtra(UsbManager.EXTRA_DEVICE)
            val res = pendingResult
            pendingResult = null
            if (res == null) return
            if (granted && device != null) {
                res.success(openFd(usbManager(), device))
            } else {
                res.success(-4) // permissão negada
            }
        }
    }
}
