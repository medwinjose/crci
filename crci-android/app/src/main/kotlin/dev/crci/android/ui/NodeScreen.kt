package dev.crci.android.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.crci.android.CrciViewModel
import dev.crci.android.ffi.validateNodeConfig
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NodeScreen(viewModel: CrciViewModel) {
    val nodeId by viewModel.nodeId.collectAsState()
    val peerCount by viewModel.peerCount.collectAsState()
    val configValid by viewModel.configValid.collectAsState()

    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            TopAppBar(
                title = { Text("CRCI") },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                    titleContentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                text = "CRCI Node",
                style = MaterialTheme.typography.headlineMedium
            )

            Card(
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text("Node ID: $nodeId")
                    Text("Peers connected: $peerCount")
                    Text("Config valid: $configValid")
                }
            }

            Button(
                onClick = {
                    val ok = validateNodeConfig(viewModel.getConfig())
                    scope.launch {
                        if (ok) {
                            snackbarHostState.showSnackbar("Config OK — message queued")
                        } else {
                            snackbarHostState.showSnackbar("Invalid config")
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Send Message")
            }

            Button(
                onClick = { viewModel.refreshPeers() },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Refresh Peers")
            }
        }
    }
}
