"""
URL configuration for liminal_salt_django project.

The `urlpatterns` list routes URLs to views. For more information please see:
    https://docs.djangoproject.com/en/6.0/topics/http/urls/
"""
from django.urls import path, include

urlpatterns = [
    path('', include('chat.urls')),
]
