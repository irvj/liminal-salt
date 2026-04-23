/**
 * Liminal Salt - Utility Functions
 * Shared helper functions used across the application.
 */

// =============================================================================
// Theme Initialization (runs immediately to prevent flash)
// =============================================================================

(function initThemeImmediate() {
    const root = document.documentElement;

    // Try localStorage first, then data attributes (server config), then defaults
    const mode = localStorage.getItem('theme') || root.dataset.themeMode || 'dark';
    const colorTheme = localStorage.getItem('colorTheme') || root.dataset.colorTheme || 'liminal-salt';
    root.setAttribute('data-theme', mode);

    // Apply cached color theme colors if available
    const cachedColors = localStorage.getItem('themeColors_' + colorTheme + '_' + mode);
    if (cachedColors) {
        try {
            const colors = JSON.parse(cachedColors);
            for (const [key, value] of Object.entries(colors)) {
                root.style.setProperty('--' + key, value);
            }
        } catch (e) {
            // Silently fail - CSS fallback colors will be used
        }
    }
})();

// =============================================================================
// HTMX CSRF Configuration
// =============================================================================

document.addEventListener('DOMContentLoaded', function() {
    document.body.addEventListener('htmx:configRequest', function(event) {
        const csrfToken = document.querySelector('meta[name="csrf-token"]')?.content;
        if (csrfToken) {
            event.detail.headers['X-CSRFToken'] = csrfToken;
        }
    });
});

// =============================================================================
// URL Configuration (read from data attributes set by Django templates)
// =============================================================================

/**
 * Get a URL from the #app-urls element's data attributes.
 * Falls back to the provided default if the element or attribute is missing.
 * @param {string} key - Data attribute name in camelCase (e.g., 'themesUrl')
 * @param {string} fallback - Default URL if attribute not found
 * @returns {string} The URL
 */
function getAppUrl(key, fallback) {
    const el = document.getElementById('app-urls');
    return (el && el.dataset[key]) || fallback;
}

// =============================================================================
// Theme Management
// =============================================================================

// Theme cache to avoid re-fetching JSON
const _loadedThemes = {};

/**
 * Get list of available color themes from the server.
 * @returns {Promise<Array>} Array of theme objects with id and name
 */
async function getAvailableThemes() {
    try {
        const response = await fetch(getAppUrl('themesUrl', '/api/themes/'));
        if (!response.ok) {
            console.error('Failed to fetch themes');
            return [{ id: 'liminal-salt', name: 'Liminal Salt' }];
        }
        const data = await response.json();
        return data.themes;
    } catch (error) {
        console.error('Error fetching themes:', error);
        // Fallback to prevent complete failure
        return [{ id: 'liminal-salt', name: 'Liminal Salt' }];
    }
}

/**
 * Save theme preference to the backend.
 * @param {string} colorTheme - Theme identifier (e.g., 'nord')
 * @param {string} themeMode - 'dark' or 'light'
 * @returns {Promise<boolean>} True if save was successful
 */
async function saveThemePreference(colorTheme, themeMode) {
    const csrfToken = getCsrfToken();
    try {
        const response = await fetch(getAppUrl('saveThemeUrl', '/api/save-theme/'), {
            method: 'POST',
            headers: {
                'Content-Type': 'application/x-www-form-urlencoded',
                'X-CSRFToken': csrfToken
            },
            body: `colorTheme=${encodeURIComponent(colorTheme)}&themeMode=${encodeURIComponent(themeMode)}`
        });
        return response.ok;
    } catch (error) {
        console.error('Failed to save theme preference:', error);
        return false;
    }
}

/**
 * Get the current color theme from localStorage or default to 'liminal-salt'.
 * @returns {string} The current color theme id
 */
function getColorTheme() {
    return localStorage.getItem('colorTheme') || 'liminal-salt';
}

/**
 * Apply theme colors to CSS custom properties.
 * @param {Object} colors - Object with color name/value pairs
 */
function applyThemeColors(colors) {
    const root = document.documentElement;
    for (const [key, value] of Object.entries(colors)) {
        root.style.setProperty(`--${key}`, value);
    }
}

/**
 * Cache theme colors in localStorage for flash-free page loads.
 * @param {string} themeId - Theme identifier
 * @param {string} mode - 'dark' or 'light'
 * @param {Object} colors - Color values to cache
 */
function cacheThemeColors(themeId, mode, colors) {
    localStorage.setItem(`themeColors_${themeId}_${mode}`, JSON.stringify(colors));
}

/**
 * Load and apply a color theme.
 * @param {string} themeId - Theme identifier (e.g., 'nord')
 * @returns {Promise} Resolves when theme is loaded and applied
 */
async function loadTheme(themeId) {
    // Fetch theme if not cached
    if (!_loadedThemes[themeId]) {
        try {
            const themesPath = getAppUrl('themesStaticPath', '/static/themes');
            const response = await fetch(`${themesPath}/${themeId}.json`);
            if (!response.ok) {
                console.error(`Failed to load theme: ${themeId}`);
                return;
            }
            _loadedThemes[themeId] = await response.json();
        } catch (error) {
            console.error(`Error loading theme ${themeId}:`, error);
            return;
        }
    }

    const theme = _loadedThemes[themeId];
    const mode = getTheme(); // 'dark' or 'light'
    const colors = theme[mode];

    if (colors) {
        applyThemeColors(colors);
        cacheThemeColors(themeId, mode, colors);
        localStorage.setItem('colorTheme', themeId);
    }
}

/**
 * Set the application theme mode (dark/light) and persist to localStorage.
 * Also re-applies current color theme with the new mode.
 * Dispatches 'theme-mode-changed' event for reactive UI updates.
 * @param {string} mode - 'dark' or 'light'
 */
function setTheme(mode) {
    document.documentElement.setAttribute('data-theme', mode);
    localStorage.setItem('theme', mode);

    // Re-apply current color theme with new mode
    const colorTheme = getColorTheme();
    loadTheme(colorTheme);

    // Dispatch event for reactive UI updates (sidebar, settings page, etc.)
    window.dispatchEvent(new CustomEvent('theme-mode-changed', { detail: { mode } }));
}

/**
 * Get the current theme from localStorage or default to 'dark'.
 * @returns {string} The current theme
 */
function getTheme() {
    return localStorage.getItem('theme') || 'dark';
}

/**
 * Initialize theme on page load (call this immediately in <head>).
 * This prevents flash of wrong theme.
 */
function initTheme() {
    const theme = getTheme();
    document.documentElement.setAttribute('data-theme', theme);
}
// =============================================================================
// CSRF Token
// =============================================================================

/**
 * Get the CSRF token from the meta tag.
 * @returns {string|null} The CSRF token or null if not found
 */
function getCsrfToken() {
    return document.querySelector('meta[name="csrf-token"]')?.content || null;
}

// =============================================================================
// Textarea Helpers
// =============================================================================

/**
 * Handle textarea keydown: Enter submits, Shift+Enter adds new line.
 * @param {KeyboardEvent} event - The keydown event
 */
function handleTextareaKeydown(event) {
    if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        event.target.form.requestSubmit();
    }
}

/**
 * Auto-resize textarea to fit content (max 200px).
 * @param {HTMLTextAreaElement} textarea - The textarea element
 */
function autoResizeTextarea(textarea) {
    textarea.style.height = 'auto';
    textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
}

// =============================================================================
// Scroll Helpers
// =============================================================================

/**
 * Scroll the messages container to the bottom.
 */
/**
 * Scroll the messages container to the bottom.
 * With flex-direction: column-reverse, scrollTop = 0 is the bottom.
 */
function scrollToBottom() {
    const messagesDiv = document.getElementById('messages');
    if (messagesDiv) {
        messagesDiv.scrollTop = 0;
    }
}

/**
 * Show/hide scroll-to-bottom button based on scroll position.
 * With column-reverse, scrollTop is negative when scrolled up from bottom.
 */
function updateScrollButtonVisibility() {
    const messagesDiv = document.getElementById('messages');
    const btn = document.getElementById('scroll-to-bottom-btn');
    if (!messagesDiv || !btn) return;

    const threshold = 200;
    const isNearBottom = Math.abs(messagesDiv.scrollTop) < threshold;

    if (isNearBottom) {
        btn.classList.add('opacity-0', 'pointer-events-none');
        btn.classList.remove('opacity-100', 'pointer-events-auto');
    } else {
        btn.classList.remove('opacity-0', 'pointer-events-none');
        btn.classList.add('opacity-100', 'pointer-events-auto');
    }
}

/**
 * Setup scroll listener for the scroll-to-bottom button.
 */
function setupScrollButtonListener() {
    const messagesDiv = document.getElementById('messages');
    if (!messagesDiv || messagesDiv._scrollBtnListenerAttached) return;

    messagesDiv.addEventListener('scroll', updateScrollButtonVisibility);
    messagesDiv._scrollBtnListenerAttached = true;

    // Initial check
    updateScrollButtonVisibility();
}

// =============================================================================
// Timezone
// =============================================================================

/**
 * Set the timezone hidden input value.
 */
function setTimezoneInput() {
    const input = document.getElementById('timezone-input');
    if (input) {
        input.value = Intl.DateTimeFormat().resolvedOptions().timeZone;
    }
}

// =============================================================================
// Timestamp & Date Formatting
// =============================================================================

/**
 * Format date for separator (Today, Yesterday, or full date).
 * @param {Date} date - The date to format
 * @returns {string} Formatted date string
 */
function formatDateSeparator(date) {
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);
    const messageDate = new Date(date.getFullYear(), date.getMonth(), date.getDate());

    if (messageDate.getTime() === today.getTime()) {
        return 'Today';
    } else if (messageDate.getTime() === yesterday.getTime()) {
        return 'Yesterday';
    } else {
        return date.toLocaleDateString([], { month: 'long', day: 'numeric', year: 'numeric' });
    }
}

/**
 * Convert UTC timestamps to local time.
 */
function convertTimestamps() {
    const timestamps = document.querySelectorAll('.timestamp[data-utc]');

    timestamps.forEach(el => {
        const utc = el.getAttribute('data-utc');
        if (!utc || el.textContent) return; // Skip if already converted

        try {
            const date = new Date(utc);
            if (isNaN(date.getTime())) return;

            // Always show time
            el.textContent = date.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
        } catch (e) {
            // Silently fail for invalid timestamps
        }
    });
}

/**
 * Insert date separators above first message of each date.
 */
function insertDateSeparators() {
    const messagesDiv = document.getElementById('messages-inner');
    if (!messagesDiv) return;

    const containers = messagesDiv.querySelectorAll('.message-container');
    let lastDateStr = null;

    containers.forEach(container => {
        const timestamp = container.querySelector('.timestamp[data-utc]');
        if (!timestamp) return;

        const utc = timestamp.getAttribute('data-utc');
        if (!utc) return;

        try {
            const date = new Date(utc);
            if (isNaN(date.getTime())) return;

            const dateStr = date.toDateString();

            if (dateStr !== lastDateStr) {
                // Check if separator already exists
                const prevSibling = container.previousElementSibling;
                if (prevSibling && prevSibling.classList.contains('date-separator')) {
                    lastDateStr = dateStr;
                    return;
                }

                // Insert new date separator
                const separator = document.createElement('div');
                separator.className = 'date-separator text-center text-foreground-muted text-xs my-6 relative';
                separator.textContent = formatDateSeparator(date);
                container.parentNode.insertBefore(separator, container);

                lastDateStr = dateStr;
            }
        } catch (e) {
            // Silently fail
        }
    });
}

// =============================================================================
// Message Helpers
// =============================================================================

/**
 * Add user message immediately and show thinking indicator.
 * @param {Event} event - The form submit event
 */
function addUserMessage(event) {
    const input = document.getElementById('message-input');
    const message = input.value.trim();

    if (!message) return;

    // Clear input immediately (don't wait for response)
    input.value = '';
    // Reset textarea height
    input.style.height = 'auto';

    // Create and append user message with container
    const messagesDiv = document.getElementById('messages-inner');
    const now = new Date();

    // Check if we need a date separator for today
    const lastContainer = messagesDiv.querySelector('.message-container:last-of-type');
    let needsSeparator = true;

    if (lastContainer) {
        const lastTimestamp = lastContainer.querySelector('.timestamp[data-utc]');
        if (lastTimestamp) {
            const lastDate = new Date(lastTimestamp.getAttribute('data-utc'));
            if (lastDate.toDateString() === now.toDateString()) {
                needsSeparator = false;
            }
        }
    } else {
        // No messages yet, check if there's already a separator
        const existingSeparator = messagesDiv.querySelector('.date-separator');
        if (existingSeparator) needsSeparator = false;
    }

    if (needsSeparator) {
        const separator = document.createElement('div');
        separator.className = 'date-separator text-center text-foreground-muted text-xs my-6 relative';
        separator.textContent = formatDateSeparator(now);
        messagesDiv.appendChild(separator);
    }

    const container = document.createElement('div');
    container.className = 'message-container user my-4 max-w-[80%] w-fit ml-auto';

    const userDiv = document.createElement('div');
    userDiv.className = 'message user message-tail-user p-3 px-4 rounded-lg bg-user-bubble text-foreground-on-accent';

    // User messages are plaintext — mirrors the server-side render in
    // chat_main.html. whitespace-pre-wrap preserves typed line breaks.
    const bodyDiv = document.createElement('div');
    bodyDiv.className = 'whitespace-pre-wrap';
    bodyDiv.textContent = message;
    userDiv.appendChild(bodyDiv);

    container.appendChild(userDiv);

    // Add controls row with edit button and timestamp
    const controlsDiv = document.createElement('div');
    controlsDiv.className = 'flex items-center gap-2 mt-3.5 px-1 justify-end';

    // Escape message for data attribute
    const escapedMessage = JSON.stringify(message).slice(1, -1);

    const editBtn = document.createElement('button');
    editBtn.type = 'button';
    editBtn.className = 'text-foreground-muted hover:text-foreground cursor-pointer';
    editBtn.dataset.action = 'edit-message';
    editBtn.addEventListener('click', () => editLastMessage(editBtn));
    editBtn.setAttribute('data-message', escapedMessage);
    editBtn.setAttribute('title', 'Edit');
    editBtn.innerHTML = '<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/></svg>';
    controlsDiv.appendChild(editBtn);

    const timestamp = document.createElement('span');
    timestamp.className = 'timestamp text-xs text-foreground-muted text-right';
    timestamp.setAttribute('data-utc', now.toISOString());
    timestamp.textContent = now.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    controlsDiv.appendChild(timestamp);

    container.appendChild(controlsDiv);
    messagesDiv.appendChild(container);

    // Clean up old edit buttons
    cleanupMessageButtons();

    // Remove ALL retry buttons - no assistant message to retry until new response arrives
    document.querySelectorAll('button[onclick="retryLastMessage()"]').forEach(btn => btn.remove());

    // Create and append thinking indicator
    const thinkingDiv = document.createElement('div');
    thinkingDiv.id = 'thinking-indicator';
    thinkingDiv.className = 'message assistant p-3 px-4 rounded-lg bg-assistant-bubble opacity-70 w-fit max-w-[80%] mr-auto my-4';
    thinkingDiv.innerHTML = '<span class="typing-indicator flex gap-1 py-1"><span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot"></span><span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot bounce-dot-2"></span><span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot bounce-dot-3"></span></span>';
    messagesDiv.appendChild(thinkingDiv);

    // Scroll to bottom to show user message and thinking indicator
    scrollToBottom();
}

/**
 * Remove the thinking indicator.
 */
function removeThinkingIndicator() {
    const thinking = document.getElementById('thinking-indicator');
    if (thinking) {
        thinking.remove();
    }
}

/**
 * Animate assistant response with typewriter effect.
 */
function animateAssistantResponse() {
    const messagesInner = document.getElementById('messages-inner');
    if (!messagesInner) return;

    // Get the last assistant message (the newly inserted one)
    const assistantMessages = messagesInner.querySelectorAll('.message.assistant:not(.thinking)');
    const lastMessage = assistantMessages[assistantMessages.length - 1];
    if (!lastMessage) return;

    // Scroll so the top of the new message bubble is visible
    const messageContainer = lastMessage.closest('.message-container');
    if (messageContainer) {
        messageContainer.scrollIntoView({ block: 'start' });
    } else {
        scrollToBottom();
    }

    // Apply typewriter to the entire message
    typewriterReveal(lastMessage);
}

/**
 * Typewriter effect - reveal text word by word.
 * @param {HTMLElement} element - The element to animate
 */
function typewriterReveal(element) {
    // Get all text nodes, but skip code blocks
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT, {
        acceptNode: function(node) {
            // Skip text inside PRE or CODE elements
            let parent = node.parentNode;
            while (parent && parent !== element) {
                if (parent.tagName === 'PRE' || parent.tagName === 'CODE') {
                    return NodeFilter.FILTER_REJECT;
                }
                parent = parent.parentNode;
            }
            return node.textContent.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
        }
    }, false);

    const textNodes = [];
    let node;
    while (node = walker.nextNode()) {
        textNodes.push(node);
    }

    // Wrap each word in a span
    const allWords = [];
    textNodes.forEach(textNode => {
        const words = textNode.textContent.split(/(\s+)/);
        const fragment = document.createDocumentFragment();

        words.forEach(word => {
            if (word.trim()) {
                const span = document.createElement('span');
                span.className = 'typewriter-word';
                span.textContent = word;
                fragment.appendChild(span);
                allWords.push(span);
            } else if (word) {
                fragment.appendChild(document.createTextNode(word));
            }
        });

        textNode.parentNode.replaceChild(fragment, textNode);
    });

    // Reveal words progressively (no auto-scroll - user controls their view)
    allWords.forEach((wordSpan, index) => {
        setTimeout(() => {
            wordSpan.classList.add('shown');
            // Update button visibility periodically
            if (index % 20 === 0) {
                updateScrollButtonVisibility();
            }
        }, index * 35);
    });

    // Final visibility check after all words revealed
    if (allWords.length > 0) {
        setTimeout(() => updateScrollButtonVisibility(), allWords.length * 35 + 100);
    }
}

// =============================================================================
// Sidebar Helpers
// =============================================================================

/**
 * Update sidebar highlight when switching sessions.
 * @param {HTMLElement} clickedButton - The clicked session button
 */
function updateSidebarHighlight(clickedButton) {
    // Remove 'current' class from all session items
    document.querySelectorAll('.session-item').forEach(item => {
        item.classList.remove('current', 'bg-accent', 'text-foreground-on-accent', 'font-bold');
        item.classList.add('bg-surface-elevated', 'text-foreground');
    });
    // Add 'current' class to clicked button
    clickedButton.classList.add('current', 'bg-accent', 'text-foreground-on-accent', 'font-bold');
    clickedButton.classList.remove('bg-surface-elevated', 'text-foreground');
}

/**
 * Clear sidebar highlight when navigating to non-chat pages.
 */
function clearSidebarHighlight() {
    document.querySelectorAll('.session-item').forEach(item => {
        item.classList.remove('current', 'bg-accent', 'text-foreground-on-accent', 'font-bold');
        item.classList.add('bg-surface-elevated', 'text-foreground');
    });
}

/**
 * Update header title after rename.
 * @param {string} newTitle - The new title
 */
function updateHeaderTitle(newTitle) {
    const headerTitle = document.getElementById('chat-title');
    if (headerTitle) {
        headerTitle.textContent = newTitle;
    }
}

// =============================================================================
// Memory Status Helpers
// =============================================================================

/**
 * Show memory updating indicator (called via HTMX hx-on::before-request).
 */
function showMemoryUpdating() {
    const status = document.getElementById('memory-status');
    const btn = document.getElementById('update-memory-btn');
    if (status) {
        status.classList.remove('hidden');
        status.innerHTML = ' · Updating Memory<span class="updating-dots"><span>.</span><span>.</span><span>.</span></span>';
    }
    if (btn) btn.disabled = true;
}

/**
 * Poll the memory update status endpoint until complete or failed.
 * @param {string} persona - Persona name
 * @param {string} statusUrl - URL for the status endpoint
 * @param {string} memoryUrl - URL to reload the memory view on completion
 */
function pollMemoryUpdateStatus(persona, statusUrl, memoryUrl) {
    // Clear any existing poll first
    if (window._memoryPollInterval) {
        clearInterval(window._memoryPollInterval);
        window._memoryPollInterval = null;
    }

    const maxDuration = 10 * 60 * 1000; // 10 minutes
    const interval = 3000; // 3 seconds
    const startTime = Date.now();
    let cancelled = false;

    function isStillViewing() {
        const bar = document.getElementById('memory-status-bar');
        return bar && bar.dataset.persona === persona;
    }

    const poll = setInterval(async () => {
        if (cancelled || !isStillViewing()) {
            clearInterval(poll);
            window._memoryPollInterval = null;
            return;
        }

        if (Date.now() - startTime > maxDuration) {
            clearInterval(poll);
            window._memoryPollInterval = null;
            const status = document.getElementById('memory-status');
            if (status) status.innerHTML = ' · Update timed out';
            const btn = document.getElementById('update-memory-btn');
            if (btn) btn.disabled = false;
            return;
        }

        let data;
        try {
            const response = await fetch(statusUrl + '?persona=' + encodeURIComponent(persona));
            data = await response.json();
        } catch (err) {
            // Network error — keep polling
            return;
        }

        // Always re-check before acting on results
        if (cancelled || !isStillViewing()) {
            clearInterval(poll);
            window._memoryPollInterval = null;
            return;
        }

        if (data.state === 'completed') {
            cancelled = true;
            clearInterval(poll);
            window._memoryPollInterval = null;
            htmx.ajax('GET', memoryUrl + '?persona=' + encodeURIComponent(persona), {target: '#main-content', swap: 'innerHTML'});
        } else if (data.state === 'failed') {
            cancelled = true;
            clearInterval(poll);
            window._memoryPollInterval = null;
            const status = document.getElementById('memory-status');
            if (status) status.innerHTML = ' · Update failed: ' + (data.error || 'Unknown error');
            const btn = document.getElementById('update-memory-btn');
            if (btn) btn.disabled = false;
        }
    }, interval);

    window._memoryPollInterval = poll;
}

/**
 * Show memory modifying indicator and clear input (called via HTMX hx-on::before-request).
 * @param {Event} event - The event
 */
function showMemoryModifying(event) {
    // Clear input immediately
    const input = document.getElementById('memory-command-input');
    if (input) input.value = '';

    // Show updating status
    const status = document.getElementById('memory-status');
    if (status) {
        status.classList.remove('hidden');
        status.innerHTML = ' · Updating Memory<span class="updating-dots"><span>.</span><span>.</span><span>.</span></span>';
    }
}

// =============================================================================
// Reusable Select Dropdown Component
// =============================================================================

/**
 * Reusable Alpine.js select dropdown component.
 * Paired with the template partial: components/select_dropdown.html
 *
 * Usage in a parent component:
 *   Embed a selectDropdown as a nested x-data, or spread its methods into your
 *   own component and call _initSelect() in your init().
 *
 * Standalone usage (template-only):
 *   <div x-data="selectDropdown" data-items='[...]' data-selected="id">
 *       {%  include 'components/select_dropdown.html' with searchable="true" %}
 *   </div>
 *
 * Dispatches 'dropdown-select' custom event with { id, label } detail.
 */
function selectDropdown() {
    return {
        // State
        items: [],
        selected: '',
        selectedLabel: '',
        open: false,
        search: '',
        highlightedIndex: 0,

        get filteredItems() {
            if (!this.search) return this.items;
            // If search matches the selected item's label, show all items
            if (this.selected) {
                const found = this.items.find(i => i.id === this.selected);
                if (found && this.search === found.label) return this.items;
            }
            const s = this.search.toLowerCase();
            return this.items.filter(i =>
                i.label.toLowerCase().includes(s) || i.id.toLowerCase().includes(s)
            );
        },

        selectItem(item) {
            this.selected = item.id;
            this.selectedLabel = item.label;
            this.search = item.label;
            this.open = false;
            this.$dispatch('dropdown-select', { id: item.id, label: item.label });
        },

        selectHighlighted() {
            if (this.filteredItems.length > 0) {
                this.selectItem(this.filteredItems[this.highlightedIndex]);
            }
        },

        highlightNext() {
            if (this.highlightedIndex < this.filteredItems.length - 1) {
                this.highlightedIndex++;
                this._scrollToHighlighted();
            }
        },

        highlightPrev() {
            if (this.highlightedIndex > 0) {
                this.highlightedIndex--;
                this._scrollToHighlighted();
            }
        },

        _scrollToHighlighted() {
            this.$nextTick(() => {
                const dropdown = this.$refs.dropdown;
                if (!dropdown) return;
                const items = dropdown.querySelectorAll('[data-dropdown-item]');
                const highlighted = items?.[this.highlightedIndex];
                if (highlighted) {
                    highlighted.scrollIntoView({ block: 'nearest' });
                }
            });
        },

        /** Set selected by ID (for programmatic updates). */
        setSelected(id) {
            this.selected = id;
            const found = this.items.find(i => i.id === id);
            this.selectedLabel = found ? found.label : '';
            this.search = found ? found.label : '';
        },

        _initSelect() {
            const el = this.$el;

            // Parse initial items and selected from data attributes
            this._syncFromAttributes();

            // Watch for reactive data-* attribute changes from parent scope.
            // Alpine's :data-items="..." updates the DOM attribute; we observe it.
            const observer = new MutationObserver(() => this._syncFromAttributes());
            observer.observe(el, { attributes: true, attributeFilter: ['data-items', 'data-selected'] });

            // Dismiss on tab-away: when focus leaves the wrapper entirely.
            // Deferred to let click handlers on dropdown items fire first.
            this.$nextTick(() => {
                const wrapper = this.$refs.dropdownWrapper;
                if (!wrapper) return;
                wrapper.addEventListener('focusout', (e) => {
                    if (!e.relatedTarget || wrapper.contains(e.relatedTarget)) return;
                    requestAnimationFrame(() => {
                        if (!wrapper.contains(document.activeElement)) {
                            this.open = false;
                        }
                    });
                });
            });
        },

        _syncFromAttributes() {
            const el = this.$el;
            let itemsChanged = false;
            if (el.dataset.items) {
                try {
                    const newItems = JSON.parse(el.dataset.items);
                    // Only update if items actually changed (avoid unnecessary resets)
                    if (JSON.stringify(newItems) !== JSON.stringify(this.items)) {
                        this.items = newItems;
                        this.highlightedIndex = 0;
                        itemsChanged = true;
                    }
                } catch (e) { /* ignore parse errors */ }
            }
            const attrSelected = el.dataset.selected ?? '';
            if (attrSelected !== this.selected) {
                this.setSelected(attrSelected);
            } else if (itemsChanged && this.selected) {
                // Items arrived after the selection was set (async load). Re-resolve
                // the label from the new list so the trigger shows the model name
                // instead of the placeholder.
                this.setSelected(this.selected);
            }
        },

        init() {
            this._initSelect();
            // Searchable mode: clear the input on open so the user can type
            // without having to delete the current selection's label first.
            // If they close without picking something new, restore the label.
            this.$watch('open', (isOpen) => {
                if (isOpen) {
                    this.search = '';
                } else if (this.search !== this.selectedLabel) {
                    this.search = this.selectedLabel;
                }
            });
        }
    };
}

// =============================================================================
// Display Name Helper
// =============================================================================

/**
 * Convert folder name to display name format.
 * @param {string} folderName - The folder/id name (e.g., 'my_persona')
 * @returns {string} Display name (e.g., 'My Persona')
 */
function toDisplayName(folderName) {
    return folderName.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

/**
 * Convert display name to folder name format.
 * @param {string} displayName - The display name (e.g., 'My Persona')
 * @returns {string} Folder name (e.g., 'my_persona')
 */
function toFolderName(displayName) {
    return displayName
        .toLowerCase()
        .replace(/\s+/g, '_')
        .replace(/[^a-z0-9_]/g, '');
}

// =============================================================================
// HTMX Error Handling
// =============================================================================

/**
 * Handle HTMX response errors by showing an error message in the messages container.
 * Removes thinking indicator and displays the error.
 * @param {Event} event - The HTMX response-error event
 */
function handleMessageError(event) {
    removeThinkingIndicator();

    const messagesInner = document.getElementById('messages-inner');
    if (!messagesInner) return;

    let errorMessage = 'Request failed. ';
    const xhr = event.detail.xhr;

    if (xhr) {
        if (xhr.status === 0) {
            errorMessage += 'The request timed out or the connection was lost. Please try again.';
        } else if (xhr.status >= 500) {
            errorMessage += `Server error (${xhr.status}). Please try again.`;
        } else {
            errorMessage += `Error ${xhr.status}: ${xhr.statusText || 'Unknown error'}`;
        }
    } else {
        errorMessage += 'The request timed out or the connection was lost. Please try again.';
    }

    // Create error message element
    const errorDiv = document.createElement('div');
    errorDiv.className = 'message-container assistant my-4 max-w-[80%] w-fit mr-auto';
    errorDiv.innerHTML = `
        <div class="message assistant message-tail-assistant bg-danger text-white p-3 px-4 rounded-lg">
            <strong class="flex items-center gap-1">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="12" y1="8" x2="12" y2="12"></line>
                    <line x1="12" y1="16" x2="12.01" y2="16"></line>
                </svg>
                Error
            </strong>
            <p class="mt-1">${errorMessage}</p>
        </div>
    `;

    messagesInner.appendChild(errorDiv);
    scrollToBottom();
}

// =============================================================================
// Draft Persistence
// =============================================================================

// Debounce timer for draft saving
let draftSaveTimer = null;

/**
 * Save draft to server (debounced).
 * Call this on textarea input events.
 */
function saveDraftDebounced() {
    // Clear any pending save
    if (draftSaveTimer) {
        clearTimeout(draftSaveTimer);
    }

    // Debounce: wait 500ms after last keystroke
    draftSaveTimer = setTimeout(() => {
        saveDraftNow();
    }, 500);
}

/**
 * Save draft immediately (no debounce).
 * Call this before session switches.
 */
function saveDraftNow() {
    const input = document.getElementById('message-input');
    const sessionIdEl = document.querySelector('[data-session-id-for-draft]');

    if (!input || !sessionIdEl) return;

    const sessionId = sessionIdEl.dataset.sessionIdForDraft;
    const draft = input.value || '';

    // Don't save empty drafts if there was nothing before
    // (but do save empty to clear a previous draft)

    fetch(getAppUrl('saveDraftUrl', '/chat/save-draft/'), {
        method: 'POST',
        headers: {
            'Content-Type': 'application/x-www-form-urlencoded',
            'X-CSRFToken': getCsrfToken()
        },
        body: `session_id=${encodeURIComponent(sessionId)}&draft=${encodeURIComponent(draft)}`
    }).catch(() => {
        // Silently ignore errors
    });
}

/**
 * Clear draft on server (cancel any pending debounced save, then save empty).
 * Call this on form submit.
 */
function clearDraft() {
    if (draftSaveTimer) {
        clearTimeout(draftSaveTimer);
        draftSaveTimer = null;
    }
    saveDraftNow();
}

/**
 * Restore draft to textarea on page load.
 * @param {string} draft - The draft text to restore
 */
function restoreDraft(draft) {
    if (!draft) return;

    const input = document.getElementById('message-input');
    if (input) {
        input.value = draft;
        autoResizeTextarea(input);
    }
}

// =============================================================================
// New Chat Draft (localStorage-based for unsaved sessions)
// =============================================================================

const NEW_CHAT_DRAFT_KEY = 'newChatDraft';

/**
 * Save new chat draft to localStorage (debounced).
 */
let newChatDraftTimer = null;
function saveNewChatDraftDebounced() {
    if (newChatDraftTimer) {
        clearTimeout(newChatDraftTimer);
    }
    newChatDraftTimer = setTimeout(() => {
        const input = document.getElementById('message-input');
        if (input) {
            localStorage.setItem(NEW_CHAT_DRAFT_KEY, input.value || '');
        }
    }, 500);
}

/**
 * Restore new chat draft from localStorage.
 */
function restoreNewChatDraft() {
    const draft = localStorage.getItem(NEW_CHAT_DRAFT_KEY);
    if (draft) {
        const input = document.getElementById('message-input');
        if (input) {
            input.value = draft;
        }
    }
}

/**
 * Clear new chat draft from localStorage.
 * Call this when a new chat is successfully started.
 */
function clearNewChatDraft() {
    localStorage.removeItem(NEW_CHAT_DRAFT_KEY);
}

/**
 * Clear any pending home-page scenario. Called on new-chat submit so the
 * scenario is consumed once the thread is created. The homePersonaPicker
 * component listens for the event and zeros its state.
 */
function clearNewChatScenario() {
    localStorage.removeItem('home-chat-scenario');
    window.dispatchEvent(new CustomEvent('home-scenario-cleared'));
}

/**
 * Copy message content to clipboard.
 * @param {HTMLElement} button - The button element with data-message attribute
 */
async function copyMessageToClipboard(button) {
    const message = button.dataset.message;
    if (!message) return;

    // Decode unicode escapes from Django's escapejs filter
    let decodedMessage;
    try {
        decodedMessage = JSON.parse('"' + message + '"');
    } catch (e) {
        decodedMessage = message;
    }

    try {
        await navigator.clipboard.writeText(decodedMessage);
    } catch (err) {
        console.error('Failed to copy message:', err);
        return;
    }

    // Brief visual feedback - swap icon temporarily
    const icon = button.querySelector('svg');
    if (icon) {
        icon.style.opacity = '0.5';
        setTimeout(() => {
            icon.style.opacity = '1';
        }, 200);
    }
}

/**
 * Retry the last assistant message.
 * Removes the last assistant response and resubmits the user message.
 */
function retryLastMessage() {
    const lastAssistantContainer = document.querySelector('.message-container.assistant:last-of-type');
    if (lastAssistantContainer) {
        const bubble = lastAssistantContainer.querySelector('.message');
        const controlsRow = lastAssistantContainer.querySelector('.flex.items-center');

        // Replace bubble with thinking indicator and reset width
        if (bubble) {
            bubble.style.width = '';  // Reset width so it shrinks to fit
            bubble.innerHTML = `
                <span class="typing-indicator flex gap-1 py-1">
                    <span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot"></span>
                    <span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot bounce-dot-2"></span>
                    <span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot bounce-dot-3"></span>
                </span>
            `;
            bubble.classList.add('opacity-70');
        }

        // Hide the controls row (copy/retry buttons)
        if (controlsRow) {
            controlsRow.style.display = 'none';
        }
    }

    htmx.ajax('POST', getAppUrl('retryUrl', '/chat/retry/'), {
        target: '.message-container.assistant:last-of-type',
        swap: 'outerHTML'
    });
}

/**
 * Edit the last user message.
 * @param {HTMLElement} button - The edit button element with data-message attribute
 */
function editLastMessage(button) {
    const content = button.dataset.message;
    // Decode unicode escapes
    let decoded;
    try {
        decoded = JSON.parse('"' + content + '"');
    } catch (e) {
        decoded = content;
    }

    const container = button.closest('.message-container');
    const bubble = container.querySelector('.message');
    const controlsRow = button.closest('.flex');

    // Store original content and width for cancel
    bubble.dataset.originalContent = bubble.innerHTML;
    controlsRow.dataset.originalContent = controlsRow.innerHTML;
    const originalWidth = bubble.offsetWidth;

    // Lock the bubble width to prevent resize
    bubble.style.width = originalWidth + 'px';

    // Replace bubble with editable textarea
    bubble.innerHTML = `
        <textarea class="w-full bg-transparent text-foreground-on-accent resize-none focus:outline-none overflow-hidden leading-[1.75] m-0 p-0 block"
                  rows="1" id="edit-message-textarea">${decoded}</textarea>
    `;

    // Replace controls row with Cancel | Save text links (aligned right like edit/time)
    controlsRow.innerHTML = `
        <span class="text-xs text-foreground-muted h-4 inline-flex items-center">
            <span onclick="cancelEdit()" class="cursor-pointer hover:text-foreground">Cancel</span>
            <span class="mx-1">|</span>
            <span onclick="saveEditedMessage()" class="cursor-pointer hover:text-foreground">Save</span>
        </span>
    `;

    // Focus the textarea and auto-size to fit content
    const textarea = bubble.querySelector('textarea');
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
    textarea.focus();
    textarea.setSelectionRange(textarea.value.length, textarea.value.length);
}

/**
 * Save the edited user message.
 */
async function saveEditedMessage() {
    const textarea = document.getElementById('edit-message-textarea');
    if (!textarea) return;

    const newContent = textarea.value.trim();
    if (!newContent) return;

    const formData = new FormData();
    formData.append('content', newContent);
    formData.append('csrfmiddlewaretoken', getCsrfToken());

    let response;
    try {
        response = await fetch(getAppUrl('editMessageUrl', '/chat/edit-message/'), {
            method: 'POST',
            body: formData
        });
    } catch (err) {
        console.error('Failed to save edited message:', err);
        cancelEdit();
        return;
    }

    if (response.ok) {
        // Reload the chat to show updated message
        htmx.ajax('GET', getAppUrl('chatUrl', '/chat/'), {
            target: '#main-content',
            swap: 'innerHTML'
        });
    } else {
        console.error('Failed to save edited message');
        cancelEdit();
    }
}

/**
 * Cancel editing and restore original message.
 */
function cancelEdit() {
    const container = document.querySelector('.message-container.user:has(#edit-message-textarea)');
    if (!container) return;

    const bubble = container.querySelector('.message');
    if (bubble && bubble.dataset.originalContent) {
        bubble.innerHTML = bubble.dataset.originalContent;
        bubble.style.width = '';  // Clear the locked width
        delete bubble.dataset.originalContent;
    }

    // Restore the controls row
    const controlsRow = container.querySelector('.flex[data-original-content]');
    if (controlsRow && controlsRow.dataset.originalContent) {
        controlsRow.className = 'flex items-center gap-2 mt-3.5 px-1 justify-end';
        controlsRow.innerHTML = controlsRow.dataset.originalContent;
        delete controlsRow.dataset.originalContent;
    }
}

/**
 * Clean up retry/edit buttons so they only appear on the most recent messages.
 * Call this after new messages are added to the chat.
 */
function cleanupMessageButtons() {
    const messages = document.getElementById('messages-inner');
    if (!messages) return;

    const allContainers = messages.querySelectorAll('.message-container');
    const assistantContainers = messages.querySelectorAll('.message-container.assistant');
    const userContainers = messages.querySelectorAll('.message-container.user');

    // Remove retry buttons from all but the last assistant message
    assistantContainers.forEach((container, index) => {
        const retryBtn = container.querySelector('button[onclick="retryLastMessage()"]');
        if (retryBtn && index < assistantContainers.length - 1) {
            retryBtn.remove();
        }
    });

    // Remove edit buttons from all but the appropriate user message
    // (last user message, or second-to-last message if last is assistant)
    const lastContainer = allContainers[allContainers.length - 1];
    const lastIsAssistant = lastContainer?.classList.contains('assistant');

    userContainers.forEach((container, index) => {
        const editBtn = container.querySelector('button[data-action="edit-message"]');
        if (!editBtn) return;

        const isLastUser = index === userContainers.length - 1;

        // Keep edit button only on the last user message
        if (!isLastUser) {
            editBtn.remove();
        }
    });
}

// =============================================================================
// Toast Notifications
// =============================================================================

/**
 * Show a transient toast notification.
 *
 * The global <div x-data="toastContainer"> in base.html listens for the
 * 'toast' window event and renders an ephemeral card in the top-right.
 *
 * @param {string} message  Text to display.
 * @param {string} [type]   'success' | 'error' | 'info' (default 'info').
 * @param {number} [duration]  Milliseconds before auto-dismiss. 0 = persistent
 *                             until the user clicks the close button.
 *                             Default 3000.
 *
 * Usage from JS:
 *   showToast('Settings saved.', 'success');
 *   showToast('Something went wrong.', 'error', 6000);
 *   showToast('Reminder stays put.', 'info', 0);
 *
 * Usage from an Alpine template (when showToast isn't in scope):
 *   @click="window.dispatchEvent(new CustomEvent('toast', {
 *       detail: { message: 'Copied!', type: 'success' }
 *   }))"
 */
function showToast(message, type = 'info', duration = 3000) {
    if (!message) return;
    window.dispatchEvent(new CustomEvent('toast', {
        detail: { message, type, duration }
    }));
}

// =============================================================================
// Confirmation Dialog
// =============================================================================

/**
 * Show a confirmation dialog and resolve when the user picks Confirm or Cancel.
 * The global <div x-data="confirmModal"> in base.html renders the dialog.
 *
 * Accepts a plain string (short form) or an options object.
 *
 * @param {string|object} messageOrOpts  Message text, or options object.
 * @param {string} [messageOrOpts.title]         Dialog title (default "Confirm").
 * @param {string} [messageOrOpts.message]       Body text.
 * @param {string} [messageOrOpts.confirmText]   Confirm-button label (default "Confirm").
 * @param {string} [messageOrOpts.cancelText]    Cancel-button label (default "Cancel").
 * @param {string} [messageOrOpts.confirmType]   'danger' (default) or 'accent' — styles the confirm button.
 * @returns {Promise<boolean>}  Resolves true on Confirm, false on Cancel / Esc / backdrop click.
 *
 * Usage:
 *   if (!await confirmDialog(`Delete ${filename}?`)) return;
 *
 *   if (!await confirmDialog({
 *       title: 'Delete Chat?',
 *       message: `This will permanently delete "${title}".`,
 *       confirmText: 'Delete',
 *   })) return;
 */
function confirmDialog(messageOrOpts) {
    const opts = typeof messageOrOpts === 'string'
        ? { message: messageOrOpts }
        : (messageOrOpts || {});
    return new Promise(resolve => {
        window.dispatchEvent(new CustomEvent('confirm-dialog', {
            detail: { ...opts, resolve }
        }));
    });
}

/**
 * Confirm-then-delete-session action. Invoked from the sidebar trash button.
 * Server swaps main content; the OOB sidebar render reflects the change.
 */
async function deleteSessionWithConfirm(sessionId, sessionTitle) {
    if (!await confirmDialog({
        title: 'Delete Chat?',
        message: `Are you sure you want to delete "${sessionTitle}"? This cannot be undone.`,
        confirmText: 'Delete',
    })) return;
    htmx.ajax('POST', getAppUrl('deleteChatUrl', '/chat/delete/'), {
        target: '#main-content',
        swap: 'innerHTML',
        values: { session_id: sessionId },
    });
}

/**
 * Confirm-then-wipe-memory action. Reads the current persona from the
 * memory view's hidden input. Server response swaps main content.
 */
async function wipeMemoryWithConfirm() {
    if (!await confirmDialog({
        title: 'Wipe Memory?',
        message: 'Are you sure you want to wipe your entire long-term memory? This action cannot be undone.',
        confirmText: 'Wipe',
    })) return;
    const personaInput = document.querySelector('input[name="persona"]');
    const persona = personaInput ? personaInput.value : '';
    try {
        const response = await fetch(getAppUrl('wipeMemoryUrl', '/memory/wipe/'), {
            method: 'POST',
            headers: {
                'X-CSRFToken': getCsrfToken(),
                'HX-Request': 'true',
                'Content-Type': 'application/x-www-form-urlencoded',
            },
            body: `persona=${encodeURIComponent(persona)}`,
        });
        const html = await response.text();
        const mainContent = document.getElementById('main-content');
        mainContent.innerHTML = html;
        htmx.process(mainContent);
    } catch (e) {
        console.error('Failed to wipe memory:', e);
        showToast('Failed to wipe memory.', 'error');
    }
}

/**
 * Confirm-then-delete-persona action. Server response swaps main content
 * and includes a success toast via its inline script.
 */
async function deletePersonaWithConfirm(persona) {
    if (!await confirmDialog({
        title: 'Delete Persona?',
        message: `Are you sure you want to delete "${toDisplayName(persona)}"? This cannot be undone.`,
        confirmText: 'Delete',
    })) return;
    try {
        const response = await fetch(getAppUrl('deletePersonaUrl', '/persona/delete/'), {
            method: 'POST',
            headers: {
                'Content-Type': 'application/x-www-form-urlencoded',
                'X-CSRFToken': getCsrfToken(),
                'HX-Request': 'true',
            },
            body: `persona=${encodeURIComponent(persona)}`,
        });
        const html = await response.text();
        const mainContent = document.getElementById('main-content');
        mainContent.innerHTML = html;
        htmx.process(mainContent);
    } catch (e) {
        console.error('Failed to delete persona:', e);
        showToast('Failed to delete persona.', 'error');
    }
}
