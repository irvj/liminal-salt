"""
URL configuration for liminal_salt_django project.

The `urlpatterns` list routes URLs to views. For more information please see:
    https://docs.djangoproject.com/en/6.0/topics/http/urls/
"""
from django.templatetags.static import static
from django.urls import path, include
from django.views.generic import RedirectView

urlpatterns = [
    path('favicon.ico', RedirectView.as_view(url=static('favicon.svg'), permanent=True)),
    path('', include('chat.urls')),
]
