package com.example.core

interface Repository<T> {
    fun findById(id: String): T?
    fun findAll(): List<T>
    fun save(entity: T): T
    fun delete(id: String)
}
