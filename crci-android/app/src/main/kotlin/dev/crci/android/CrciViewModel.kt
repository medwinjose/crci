package dev.crci.android

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.crci.android.ffi.FfiNodeConfig
import dev.crci.android.ffi.peerCount
import dev.crci.android.ffi.startNode
import dev.crci.android.ffi.stopNode
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class CrciViewModel : ViewModel() {

    // Sealed state for node lifecycle
    sealed class NodeStatus {
        object Stopped : NodeStatus()
        object Starting : NodeStatus()
        object Running : NodeStatus()
        object Error : NodeStatus()
    }

    private val _nodeStatus = MutableStateFlow<NodeStatus>(NodeStatus.Stopped)
    val nodeStatus: StateFlow<NodeStatus> = _nodeStatus.asStateFlow()

    private val _peerCount = MutableStateFlow(0)
    val peerCount: StateFlow<Int> = _peerCount.asStateFlow()

    private val _nodeId = MutableStateFlow("")
    val nodeId: StateFlow<String> = _nodeId.asStateFlow()

    // Called from MainActivity after System.loadLibrary succeeds
    fun startNode() {
        viewModelScope.launch(Dispatchers.IO) {
            _nodeStatus.value = NodeStatus.Starting
            val config = FfiNodeConfig(
                nodeId = "crci-android-node-1",
                listenAddr = "0.0.0.0:9000",
                maxPeers = 8u
            )
            val ok = startNode(config)   // FFI call
            _nodeId.value = config.nodeId
            _nodeStatus.value = if (ok) NodeStatus.Running else NodeStatus.Error
            if (ok) startPolling()
        }
    }

    fun stopNode() {
        pollingJob?.cancel()
        viewModelScope.launch(Dispatchers.IO) {
            stopNode()   // FFI call
            _nodeStatus.value = NodeStatus.Stopped
            _peerCount.value = 0
        }
    }

    private var pollingJob: Job? = null

    private fun startPolling() {
        pollingJob = viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                _peerCount.value = peerCount().toInt()   // FFI call
                delay(5_000)
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        stopNode()
    }
}
