package com.example.core

class ReportServiceImpl {
    fun generateReport(id: String): String = "class: $id"
}

// Top-level function that calls the other top-level function generateReport
fun useTopLevel() = generateReport("test")
