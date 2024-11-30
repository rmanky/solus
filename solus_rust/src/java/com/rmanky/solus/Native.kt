package com.rmanky.solus

object Native {
    init {
        System.loadLibrary("rust")
    }

    external fun startRustServer(
        replicateApiKey: String,
        geminiApiKey: String
    ): Boolean
}