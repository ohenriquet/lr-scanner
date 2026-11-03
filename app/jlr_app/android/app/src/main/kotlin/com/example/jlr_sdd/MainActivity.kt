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
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

/**
 * Abre o Mongoose Pro JLR por USB-OTG e devolve o file descriptor do
 * UsbManager para o núcleo nativo (Rust) usar via nusb. EXPERIMENTAL —
 * precisa de validação no dispositivo.
 */
class MainActivity : FlutterActivity() {
    private val channelName = "jlr/usb"
    private val actionPermission = "com.example.jlr_sdd.USB_PERMISSION"

    // Mongoose Pro JLR (Drew Technologies): VID 0x18E1, PID 0x0104.
    private val vid = 0x18E1
    private val pid = 0x0104

    private var pendingResult: MethodChannel.Result? = null
    // Mantém a conexão viva; o fd é duplicado pelo lado nativo.
    private var openConnection: UsbDeviceConnection? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, channelName)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "openMongoose" -> openMongoose(result)
                    "close" -> {
                        try {
                            openConnection?.close()
                        } catch (_: Exception) {
                        }
                        openConnection = null
                        result.success(true)
                    }
                    else -> result.notImplemented()
                }
            }
    }

    private fun usbManager(): UsbManager =
        getSystemService(Context.USB_SERVICE) as UsbManager

    private fun findDevice(): UsbDevice? =
        usbManager().deviceList.values.firstOrNull {
            it.vendorId == vid && it.productId == pid
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
        pendingResult = result
        val flags =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S)
                PendingIntent.FLAG_MUTABLE
            else 0
        val pi = PendingIntent.getBroadcast(
            this, 0, Intent(actionPermission).setPackage(packageName), flags
        )
        val filter = IntentFilter(actionPermission)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(permissionReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            registerReceiver(permissionReceiver, filter)
        }
        mgr.requestPermission(device, pi)
    }

    private fun openFd(mgr: UsbManager, device: UsbDevice): Int {
        return try {
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
                unregisterReceiver(this)
            } catch (_: Exception) {
            }
            val granted =
                intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
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
