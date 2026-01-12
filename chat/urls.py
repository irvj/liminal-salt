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
    path('chat/start/', views.start_chat, name='start_chat'),
    path('chat/switch/', views.switch_session, name='switch_session'),
    path('chat/delete/', views.delete_chat, name='delete_chat'),
    path('chat/pin/', views.toggle_pin_chat, name='toggle_pin_chat'),
    path('chat/rename/', views.rename_chat, name='rename_chat'),
    path('chat/send/', views.send_message, name='send_message'),
    path('memory/', views.memory, name='memory'),
    path('memory/update/', views.update_memory, name='update_memory'),
    path('memory/wipe/', views.wipe_memory, name='wipe_memory'),
    path('memory/modify/', views.modify_memory, name='modify_memory'),
    path('memory/context/upload/', views.upload_context_file, name='upload_context_file'),
    path('memory/context/delete/', views.delete_context_file, name='delete_context_file'),
    path('memory/context/toggle/', views.toggle_context_file, name='toggle_context_file'),
    path('memory/context/content/', views.get_context_file_content, name='get_context_file_content'),
    path('memory/context/save/', views.save_context_file_content, name='save_context_file_content'),
    path('settings/', views.settings, name='settings'),
    path('settings/save/', views.save_settings, name='save_settings'),
    path('settings/validate-api-key/', views.validate_provider_api_key, name='validate_provider_api_key'),
    path('settings/save-provider-model/', views.save_provider_model, name='save_provider_model'),
    path('settings/save-personality/', views.save_personality_file, name='save_personality_file'),
    path('settings/create-personality/', views.create_personality, name='create_personality'),
    path('settings/delete-personality/', views.delete_personality, name='delete_personality'),
    path('settings/save-personality-model/', views.save_personality_model, name='save_personality_model'),
]
