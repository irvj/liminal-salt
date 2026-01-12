/**
 * Liminal Salt - Utility Functions
 * Shared helper functions used across the application.
 */

// =============================================================================
// Theme Management
// =============================================================================

/**
 * Set the application theme and persist to localStorage.
 * @param {string} theme - 'dark' or 'light'
 */
function setTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
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

/**
 * Update theme toggle buttons to reflect current state.
 * @param {string} theme - 'dark' or 'light'
 */
function updateThemeButtons(theme) {
    const darkBtn = document.getElementById('theme-dark-btn');
    const lightBtn = document.getElementById('theme-light-btn');
    if (!darkBtn || !lightBtn) return;

    if (theme === 'dark') {
        darkBtn.classList.add('bg-accent', 'text-foreground-on-accent');
        darkBtn.classList.remove('bg-surface-elevated');
        lightBtn.classList.remove('bg-accent', 'text-foreground-on-accent');
        lightBtn.classList.add('bg-surface-elevated');
    } else {
        lightBtn.classList.add('bg-accent', 'text-foreground-on-accent');
        lightBtn.classList.remove('bg-surface-elevated');
        darkBtn.classList.remove('bg-accent', 'text-foreground-on-accent');
        darkBtn.classList.add('bg-surface-elevated');
    }
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
