package dev.crci.android

import androidx.lifecycle.ViewModel
import dev.crci.android.ffi.FfiNodeConfig
import dev.crci.android.ffi.listPeersStub
import dev.crci.android.ffi.validateNodeConfig
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class CrciViewModel : ViewModel() {

    private val config = FfiNodeConfig(
        nodeId = "crci-android-node-1",
        listenAddr = "0.0.0.0:9000",
        maxPeers = 8u
    )

    private val _nodeId = MutableStateFlow(config.nodeId)
    val nodeId: StateFlow<String> = _nodeId.asStateFlow()

    private val _peerCount = MutableStateFlow(listPeersStub().size)
    val peerCount: StateFlow<Int> = _peerCount.asStateFlow()

    private val _configValid = MutableStateFlow(validateNodeConfig(config))
    val configValid: StateFlow<Boolean> = _configValid.asStateFlow()

    fun refreshPeers() {
        _peerCount.value = listPeersStub().size
    }

    fun getConfig(): FfiNodeConfig = config
}
