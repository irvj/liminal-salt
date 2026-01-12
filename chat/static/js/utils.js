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
        const response = await fetch('/api/themes/');
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
        const response = await fetch('/api/save-theme/', {
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
            const response = await fetch(`/static/themes/${themeId}.json`);
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
function scrollToBottom() {
    const messagesDiv = document.getElementById('messages');
    if (messagesDiv) {
        messagesDiv.scrollTop = messagesDiv.scrollHeight;
    }
}

/**
 * Show/hide scroll-to-bottom button based on scroll position.
 */
function updateScrollButtonVisibility() {
    const messagesDiv = document.getElementById('messages');
    const btn = document.getElementById('scroll-to-bottom-btn');
    if (!messagesDiv || !btn) return;

    const threshold = 200; // pixels from bottom
    const isNearBottom = messagesDiv.scrollHeight - messagesDiv.scrollTop - messagesDiv.clientHeight < threshold;

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
    const messagesDiv = document.getElementById('messages');
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
    const messagesDiv = document.getElementById('messages');
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
    userDiv.className = 'message user message-tail-user p-3 px-4 rounded-lg bg-user-bubble text-foreground-on-accent whitespace-pre-wrap';
    userDiv.textContent = message;
    container.appendChild(userDiv);

    // Add timestamp outside the bubble
    const timestamp = document.createElement('span');
    timestamp.className = 'timestamp block text-xs text-foreground-muted mt-3.5 px-1 text-right';
    timestamp.setAttribute('data-utc', now.toISOString());
    timestamp.textContent = now.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    container.appendChild(timestamp);

    messagesDiv.appendChild(container);

    // Create and append thinking indicator
    const thinkingDiv = document.createElement('div');
    thinkingDiv.id = 'thinking-indicator';
    thinkingDiv.className = 'message assistant p-3 px-4 rounded-lg bg-assistant-bubble opacity-70 w-fit max-w-[80%] mr-auto my-4';
    thinkingDiv.innerHTML = '<span class="typing-indicator flex gap-1 py-1"><span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot"></span><span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot" style="animation-delay: 0.2s;"></span><span class="w-2 h-2 bg-foreground-secondary rounded-full animate-bounce-dot" style="animation-delay: 0.4s;"></span></span>';
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
    const messagesDiv = document.getElementById('messages');
    if (!messagesDiv) return;

    // Scroll to show the new response (once, before typewriter starts)
    scrollToBottom();

    // Get the last assistant message (the newly inserted one)
    const assistantMessages = messagesDiv.querySelectorAll('.message.assistant:not(.thinking)');
    const lastMessage = assistantMessages[assistantMessages.length - 1];
    if (!lastMessage) return;

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
        }, index * 25);
    });

    // Final visibility check after all words revealed
    if (allWords.length > 0) {
        setTimeout(() => updateScrollButtonVisibility(), allWords.length * 25 + 100);
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
        status.style.display = 'inline';
        status.innerHTML = ' · Updating Memory<span class="updating-dots"><span>.</span><span>.</span><span>.</span></span>';
    }
    if (btn) btn.disabled = true;
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
        status.style.display = 'inline';
        status.innerHTML = ' · Updating Memory<span class="updating-dots"><span>.</span><span>.</span><span>.</span></span>';
    }
}

// =============================================================================
// Dropdown Scroll Helper
// =============================================================================

/**
 * Scroll highlighted item into view in dropdown.
 * Used by searchable dropdown components.
 * @param {HTMLElement} root - The root element containing the dropdown
 * @param {number} highlightedIndex - The index of the highlighted item
 */
function scrollDropdownToHighlighted(root, highlightedIndex) {
    const dropdown = root.querySelector('.max-h-64');
    const buttons = dropdown?.querySelectorAll('button');
    const highlighted = buttons?.[highlightedIndex];
    if (dropdown && highlighted) {
        const itemTop = highlighted.offsetTop;
        const itemBottom = itemTop + highlighted.offsetHeight;
        const viewTop = dropdown.scrollTop;
        const viewBottom = viewTop + dropdown.clientHeight;

        if (itemBottom > viewBottom) {
            dropdown.scrollTop = itemBottom - dropdown.clientHeight;
        } else if (itemTop < viewTop) {
            dropdown.scrollTop = itemTop;
        }
    }
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

    const messagesDiv = document.getElementById('messages');
    if (!messagesDiv) return;

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

    messagesDiv.appendChild(errorDiv);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
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

    fetch('/chat/save-draft/', {
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
