package com.example.feature

import com.example.core.User
import com.example.core.UserRole
import com.example.core.UserService
import com.example.core.isAdmin

class UserProfile(private val userService: UserService) {

    fun getProfile(userId: String): ProfileData? {
        val user = userService.getUser(userId) ?: return null
        return ProfileData(
            displayName = "${user.name}",
            email = user.email,
            isAdmin = user.isAdmin,
            role = user.role
        )
    }

    data class ProfileData(
        val displayName: String,
        val email: String,
        val isAdmin: Boolean,
        val role: UserRole
    )
}
