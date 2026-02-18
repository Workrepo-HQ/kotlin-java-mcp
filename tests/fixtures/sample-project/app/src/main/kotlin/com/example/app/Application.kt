package com.example.app

import com.example.core.User
import com.example.core.UserRole
import com.example.core.UserService
import com.example.core.displayName

class Application(private val userService: UserService) {

    fun run() {
        val admin = userService.createUser("Admin", "admin@example.com", UserRole.ADMIN)
        println("Created user: ${admin.displayName()}")

        val allUsers = userService.getAllUsers()
        println("Total users: ${allUsers.size}")
    }
}
