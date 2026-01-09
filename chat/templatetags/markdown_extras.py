from django import template
from django.utils.safestring import mark_safe
import markdown

register = template.Library()


@register.filter(name='markdown')
def markdown_filter(value):
    if value:
        return mark_safe(markdown.markdown(value))
    return ''


@register.filter(name='display_name')
def display_name_filter(value):
    """Convert folder name to display: 'the_assistant' -> 'The Assistant'"""
    if value:
        return value.replace('_', ' ').title()
    return ''
