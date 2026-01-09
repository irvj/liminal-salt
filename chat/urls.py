"""
URL configuration for chat app.
"""
from django.urls import path
from . import views

urlpatterns = [
    path('', views.index, name='index'),
    path('setup/', views.setup_wizard, name='setup'),
    path('chat/', views.chat, name='chat'),
    path('chat/new/', views.new_chat, name='new_chat'),
    path('chat/switch/', views.switch_session, name='switch_session'),
    path('chat/delete/', views.delete_chat, name='delete_chat'),
    path('chat/send/', views.send_message, name='send_message'),
    path('memory/', views.memory, name='memory'),
    path('memory/update/', views.update_memory, name='update_memory'),
    path('memory/wipe/', views.wipe_memory, name='wipe_memory'),
    path('memory/modify/', views.modify_memory, name='modify_memory'),
    path('settings/', views.settings, name='settings'),
    path('settings/save/', views.save_settings, name='save_settings'),
    path('settings/save-personality/', views.save_personality_file, name='save_personality_file'),
]
