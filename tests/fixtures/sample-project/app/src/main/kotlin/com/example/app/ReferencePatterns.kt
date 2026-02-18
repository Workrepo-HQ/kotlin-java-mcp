package com.example.app

import com.example.core.Config
import com.example.core.User
import com.example.core.createUser
import com.example.core.mapTo

class ReferencePatterns {

    // 1. Navigation receivers: Config is the receiver, should be captured as a reference
    fun useConfig() {
        val retries = Config.maxRetries
        val timeout = Config.getTimeout()
    }

    // 2. Callable references: ::createUser and User::toString
    fun useCallableRefs() {
        val factory = ::createUser
        val names = listOf("Alice", "Bob").map(::createUser)
        val toStr = User::toString
    }

    // 3. Infix function calls: mapTo is an infix function
    fun useInfix() {
        val pair = "key" mapTo "value"
    }
}
