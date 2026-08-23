package dev.crci.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.Assert.assertTrue
import org.junit.Assert.assertThrows
import dev.crci.android.ffi.*

@RunWith(AndroidJUnit4::class)
class Bft038PanicBoundaryTest {
    @Test
    fun testBft038_FfiPanicDoesNotCrashJvm() {
        val exception = assertThrows(Exception::class.java) {
            forcePanicForBft038()
        }
        
        assertTrue(exception.message?.contains("panic") == true || exception is InternalException)
        
        val version = crciVersion()
        assertTrue(version == "0.1.0")
    }
}
