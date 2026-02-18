package com.example.app

import com.example.core.LombokUser

class LombokUsage {
    fun printUser(user: LombokUser) {
        println(user.username)
        println(user.email)
        println(user.isActive)
    }

    fun updateUser(user: LombokUser) {
        user.username = "updated"
        user.email = "updated@example.com"
    }
}
