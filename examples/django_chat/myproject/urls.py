from chat import views
from django.contrib import admin
from django.urls import path

urlpatterns = [
    path("admin/", admin.site.urls),
    path("", views.index, name="index"),
    path("api/broadcast/", views.broadcast_message, name="broadcast"),
]
