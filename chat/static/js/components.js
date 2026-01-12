/**
 * Liminal Salt - Alpine.js Components
 * All Alpine components are registered here using Alpine.data().
 */

// =============================================================================
// Component Registration
// =============================================================================

document.addEventListener('alpine:init', () => {
    // Reusable Components
    Alpine.data('searchableDropdown', searchableDropdown);
    Alpine.data('collapsibleSection', collapsibleSection);

    // Modal Components
    Alpine.data('deleteModal', deleteModal);
    Alpine.data('renameModal', renameModal);
    Alpine.data('wipeMemoryModal', wipeMemoryModal);
    Alpine.data('editPersonaModal', editPersonaModal);
    Alpine.data('deletePersonaModal', deletePersonaModal);
    Alpine.data('editPersonaModelModal', editPersonaModelModal);
    Alpine.data('contextFilesModal', contextFilesModal);

    // Page Components
    Alpine.data('sidebarState', sidebarState);
    Alpine.data('providerModelSettings', providerModelSettings);
    Alpine.data('homePersonaPicker', homePersonaPicker);
    Alpine.data('personaSettingsPicker', personaSettingsPicker);
    Alpine.data('providerPicker', providerPicker);
    Alpine.data('modelPicker', modelPicker);
});

// =============================================================================
// Reusable: Searchable Dropdown
// =============================================================================

/**
 * Reusable searchable dropdown with keyboard navigation.
 * @param {Object} config - Configuration object
 * @param {string} config.initial - Initial selected value
 * @param {Array} config.items - Array of {id, display} objects
 * @param {Function} config.onSelect - Callback when item is selected
 */
function searchableDropdown(config = {}) {
    return {
        open: false,
        search: '',
        selected: config.initial || '',
        selectedDisplay: '',
        highlightedIndex: 0,
        items: config.items || [],
        onSelectCallback: config.onSelect || null,

        get filteredItems() {
            if (!this.search || this.search === this.selectedDisplay) return this.items;
            const s = this.search.toLowerCase();
            return this.items.filter(item =>
                item.display.toLowerCase().includes(s) || item.id.toLowerCase().includes(s)
            );
        },

        selectItem(item) {
            this.selected = item.id;
            this.selectedDisplay = item.display;
            this.search = item.display;
            this.open = false;
            if (this.onSelectCallback) {
                this.onSelectCallback(item);
            }
        },

        selectHighlighted() {
            if (this.filteredItems.length > 0) {
                this.selectItem(this.filteredItems[this.highlightedIndex]);
            }
        },

        highlightNext() {
            if (this.highlightedIndex < this.filteredItems.length - 1) {
                this.highlightedIndex++;
                this.scrollToHighlighted();
            }
        },

        highlightPrev() {
            if (this.highlightedIndex > 0) {
                this.highlightedIndex--;
                this.scrollToHighlighted();
            }
        },

        scrollToHighlighted() {
            this.$nextTick(() => {
                scrollDropdownToHighlighted(this.$root, this.highlightedIndex);
            });
        },

        init() {
            const found = this.items.find(item => item.id === this.selected);
            if (found) {
                this.selectedDisplay = found.display;
                this.search = found.display;
            }
        }
    };
}

// =============================================================================
// Reusable: Collapsible Section
// =============================================================================

/**
 * Simple collapsible section toggle.
 * @param {boolean} initiallyOpen - Whether the section starts open
 */
function collapsibleSection(initiallyOpen = true) {
    return {
        open: initiallyOpen
    };
}

// =============================================================================
// Sidebar State Component
// =============================================================================

function sidebarState() {
    return {
        collapsed: localStorage.getItem('sidebarCollapsed') === 'true',
        isMobile: window.innerWidth < 1024,
        isDark: localStorage.getItem('theme') !== 'light',

        toggleTheme() {
            this.isDark = !this.isDark;
            setTheme(this.isDark ? 'dark' : 'light');
        },

        init() {
            // Auto-collapse on smaller screens (< 1024px / lg breakpoint)
            if (this.isMobile) this.collapsed = true;

            // Listen for resize
            window.addEventListener('resize', () => {
                const wasMobile = this.isMobile;
                this.isMobile = window.innerWidth < 1024;
                // Auto-collapse when entering mobile
                if (this.isMobile && !wasMobile) this.collapsed = true;
                // Restore localStorage state when returning to desktop
                if (!this.isMobile && wasMobile) {
                    this.collapsed = localStorage.getItem('sidebarCollapsed') === 'true';
                }
            });

            // Persist collapsed state (desktop only)
            this.$watch('collapsed', val => {
                if (!this.isMobile) localStorage.setItem('sidebarCollapsed', val);
            });
        }
    };
}

// =============================================================================
// Delete Modal Component
// =============================================================================

function deleteModal() {
    return {
        showModal: false,
        sessionId: '',
        sessionTitle: '',

        init() {
            // Store reference so openDeleteModal can access this
            window.deleteModalComponent = this;
        },

        open(sessionId, sessionTitle) {
            this.sessionId = sessionId;
            this.sessionTitle = sessionTitle;
            this.showModal = true;
        }
    };
}

// Global helper function
function openDeleteModal(sessionId, sessionTitle) {
    if (window.deleteModalComponent) {
        window.deleteModalComponent.open(sessionId, sessionTitle);
    }
}

// =============================================================================
// Rename Modal Component
// =============================================================================

function renameModal() {
    return {
        showModal: false,
        sessionId: '',
        newTitle: '',

        init() {
            window.renameModalComponent = this;
        },

        open(sessionId, currentTitle) {
            this.sessionId = sessionId;
            // Get current title from header (may have been updated dynamically)
            const headerTitle = document.getElementById('chat-title');
            this.newTitle = headerTitle ? headerTitle.textContent : currentTitle;
            this.showModal = true;
        }
    };
}

// Global helper function
function openRenameModal(sessionId, currentTitle) {
    if (window.renameModalComponent) {
        window.renameModalComponent.open(sessionId, currentTitle);
    }
}

// =============================================================================
// Wipe Memory Modal Component
// =============================================================================

function wipeMemoryModal() {
    return {
        showModal: false,
        wipeUrl: '',

        init() {
            window.wipeMemoryModalComponent = this;
            // Get URL from data attribute
            this.wipeUrl = this.$el.dataset.wipeUrl || '/memory/wipe/';
        },

        open() {
            this.showModal = true;
        },

        confirmWipe() {
            this.showModal = false;
            const csrfToken = getCsrfToken();

            // Send wipe request via HTMX-style fetch
            fetch(this.wipeUrl, {
                method: 'POST',
                headers: {
                    'X-CSRFToken': csrfToken,
                    'HX-Request': 'true'
                }
            }).then(response => response.text())
            .then(html => {
                const mainContent = document.getElementById('main-content');
                mainContent.innerHTML = html;
                htmx.process(mainContent);
            });
        }
    };
}

// Global helper function
function openWipeMemoryModal() {
    if (window.wipeMemoryModalComponent) {
        window.wipeMemoryModalComponent.open();
    }
}

// =============================================================================
// Edit Persona Modal Component
// =============================================================================

function editPersonaModal() {
    return {
        showModal: false,
        isNew: false,         // true for new persona, false for edit
        isAssistant: false,   // true if editing the "assistant" persona (cannot rename)
        persona: '',          // Original folder name (empty for new)
        displayName: '',      // User-editable display name
        content: '',
        createUrl: '',
        saveUrl: '',

        init() {
            window.editPersonaModalComponent = this;
            // Get URLs from data attributes
            this.createUrl = this.$el.dataset.createUrl || '/settings/create-persona/';
            this.saveUrl = this.$el.dataset.saveUrl || '/settings/save-persona/';
        },

        openNew() {
            this.isNew = true;
            this.isAssistant = false;
            this.persona = '';
            this.displayName = '';
            this.content = '';
            this.showModal = true;
        },

        openEdit(persona, content) {
            this.isNew = false;
            this.isAssistant = (persona === 'assistant');
            this.persona = persona;
            this.displayName = toDisplayName(persona);
            this.content = content;
            this.showModal = true;
        },

        savePersona() {
            const csrfToken = getCsrfToken();

            // Convert display name to folder name format
            const newFolderName = toFolderName(this.displayName);

            const url = this.isNew ? this.createUrl : this.saveUrl;
            // Don't send new_name for assistant persona (cannot be renamed)
            const body = this.isNew
                ? `name=${encodeURIComponent(newFolderName)}&content=${encodeURIComponent(this.content)}`
                : this.isAssistant
                    ? `persona=${encodeURIComponent(this.persona)}&content=${encodeURIComponent(this.content)}`
                    : `persona=${encodeURIComponent(this.persona)}&new_name=${encodeURIComponent(newFolderName)}&content=${encodeURIComponent(this.content)}`;

            fetch(url, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                    'X-CSRFToken': csrfToken,
                    'HX-Request': 'true'
                },
                body: body
            })
            .then(response => response.text())
            .then(html => {
                const mainContent = document.getElementById('main-content');
                mainContent.innerHTML = html;
                htmx.process(mainContent);  // Re-initialize HTMX on new content
                this.showModal = false;
            });
        }
    };
}

// Global helper functions
function openEditPersonaModal() {
    if (window.editPersonaModalComponent) {
        const select = document.getElementById('persona');
        const persona = select ? select.value : '';

        // Read content from template element (survives HTMX swaps, preserves formatting)
        const contentTemplate = document.getElementById('persona-raw-content');
        const content = contentTemplate ? contentTemplate.innerHTML : '';

        window.editPersonaModalComponent.openEdit(persona, content);
    }
}

function openNewPersonaModal() {
    if (window.editPersonaModalComponent) {
        window.editPersonaModalComponent.openNew();
    }
}

// =============================================================================
// Delete Persona Modal Component
// =============================================================================

function deletePersonaModal() {
    return {
        showModal: false,
        persona: '',
        displayName: '',
        deleteUrl: '',

        init() {
            window.deletePersonaModalComponent = this;
            this.deleteUrl = this.$el.dataset.deleteUrl || '/settings/delete-persona/';
        },

        open(persona) {
            this.persona = persona;
            this.displayName = toDisplayName(persona);
            this.showModal = true;
        },

        confirmDelete() {
            const csrfToken = getCsrfToken();

            fetch(this.deleteUrl, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                    'X-CSRFToken': csrfToken,
                    'HX-Request': 'true'
                },
                body: `persona=${encodeURIComponent(this.persona)}`
            })
            .then(response => response.text())
            .then(html => {
                const mainContent = document.getElementById('main-content');
                mainContent.innerHTML = html;
                htmx.process(mainContent);
                this.showModal = false;
            });
        }
    };
}

// Global helper function
function openDeletePersonaModal() {
    if (window.deletePersonaModalComponent) {
        const select = document.getElementById('persona');
        const persona = select ? select.value : '';
        window.deletePersonaModalComponent.open(persona);
    }
}

// =============================================================================
// Edit Persona Model Modal Component
// =============================================================================

function editPersonaModelModal() {
    return {
        showModal: false,
        persona: '',
        displayName: '',
        currentModel: '',
        selectedModel: '',
        modelSearch: '',
        models: [],
        modelsLoaded: false,
        loading: false,
        loadError: '',
        defaultModel: '',
        modelOpen: false,
        highlightedIndex: 0,
        statusMessage: '',
        statusType: '',
        saving: false,
        modelsUrl: '',
        saveUrl: '',

        init() {
            window.editPersonaModelModalComponent = this;
            this.modelsUrl = this.$el.dataset.modelsUrl || '/settings/available-models/';
            this.saveUrl = this.$el.dataset.saveUrl || '/settings/save-persona-model/';
        },

        async loadModels() {
            if (this.modelsLoaded || this.loading) return;

            this.loading = true;
            this.loadError = '';

            try {
                const response = await fetch(this.modelsUrl);
                const data = await response.json();

                if (response.ok && data.models) {
                    this.models = data.models;
                    this.modelsLoaded = true;

                    // Update modelSearch with current model display name
                    if (this.currentModel) {
                        const model = this.models.find(m => m.id === this.currentModel);
                        this.modelSearch = model ? model.display : this.currentModel;
                    }
                } else {
                    this.loadError = data.error || 'Failed to load models';
                }
            } catch (e) {
                this.loadError = 'Failed to fetch models. Please try again.';
            } finally {
                this.loading = false;
            }
        },

        get filteredModels() {
            if (!this.modelSearch) return this.models;
            const s = this.modelSearch.toLowerCase();
            return this.models.filter(m =>
                m.display.toLowerCase().includes(s) || m.id.toLowerCase().includes(s)
            );
        },

        selectModel(model) {
            this.selectedModel = model.id;
            this.modelSearch = model.display;
            this.modelOpen = false;
        },

        selectHighlighted() {
            if (this.filteredModels.length > 0) {
                this.selectModel(this.filteredModels[this.highlightedIndex]);
            }
        },

        highlightNext() {
            if (this.highlightedIndex < this.filteredModels.length - 1) {
                this.highlightedIndex++;
            }
        },

        highlightPrev() {
            if (this.highlightedIndex > 0) {
                this.highlightedIndex--;
            }
        },

        clearModel() {
            this.selectedModel = '';
            this.modelSearch = '';
        },

        open(persona, personaModel, defaultModel) {
            this.persona = persona;
            this.displayName = toDisplayName(persona);
            this.currentModel = personaModel;
            this.selectedModel = personaModel;
            this.defaultModel = defaultModel;
            this.statusMessage = '';
            this.loadError = '';
            this.modelSearch = personaModel || '';
            this.showModal = true;

            // Fetch models lazily when modal opens
            this.loadModels();
        },

        async saveModel() {
            this.saving = true;
            this.statusMessage = '';

            const csrfToken = getCsrfToken();

            try {
                const formData = new FormData();
                formData.append('persona', this.persona);
                formData.append('model', this.selectedModel);

                const response = await fetch(this.saveUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': csrfToken },
                    body: formData
                });
                const data = await response.json();

                if (data.success) {
                    this.statusMessage = 'Model updated successfully!';
                    this.statusType = 'success';
                    this.currentModel = data.model || '';

                    // Refresh persona page to show updated model
                    setTimeout(() => {
                        this.showModal = false;
                        htmx.ajax('GET', '/persona/?preview=' + this.persona, {target: '#main-content', swap: 'innerHTML'});
                    }, 1000);
                } else {
                    this.statusMessage = data.error || 'Failed to save model';
                    this.statusType = 'error';
                }
            } catch (e) {
                this.statusMessage = 'Failed to save model. Please try again.';
                this.statusType = 'error';
            } finally {
                this.saving = false;
            }
        }
    };
}

// Global helper function
function openEditPersonaModelModal() {
    if (window.editPersonaModelModalComponent) {
        // Read from data attributes (survives HTMX swaps)
        const personaData = document.getElementById('persona-data');
        const persona = personaData ? personaData.dataset.selectedId : '';
        const personaModel = personaData ? personaData.dataset.personaModel : '';
        const defaultModel = personaData ? personaData.dataset.defaultModel : '';

        window.editPersonaModelModalComponent.open(persona, personaModel, defaultModel);
    }
}

// =============================================================================
// Context Files Modal Component
// =============================================================================

function contextFilesModal() {
    return {
        showModal: false,
        isDragging: false,
        files: [],
        uploadStatus: '',
        uploadStatusType: '',
        editModal: {
            show: false,
            filename: '',
            content: '',
            status: '',
            statusType: ''
        },

        init() {
            window.contextFilesModalComponent = this;
            this.loadFiles();
        },

        loadFiles() {
            this.files = window.contextFilesData || [];
        },

        handleDrop(event) {
            this.isDragging = false;
            const files = event.dataTransfer.files;
            this.uploadFiles(files);
        },

        handleFileSelect(event) {
            const files = event.target.files;
            this.uploadFiles(files);
            event.target.value = '';
        },

        async uploadFiles(fileList) {
            for (const file of fileList) {
                if (!file.name.endsWith('.md') && !file.name.endsWith('.txt')) {
                    this.showStatus(`${file.name}: Invalid file type`, 'error');
                    continue;
                }

                const formData = new FormData();
                formData.append('file', file);
                formData.append('csrfmiddlewaretoken', document.querySelector('[name=csrfmiddlewaretoken]').value);

                try {
                    const response = await fetch('/memory/context/upload/', {
                        method: 'POST',
                        body: formData,
                        headers: {
                            'X-Requested-With': 'XMLHttpRequest'
                        }
                    });

                    if (response.ok) {
                        const data = await response.json();
                        this.files = data.files;
                        window.contextFilesData = data.files;
                        this.showStatus(`Uploaded ${file.name}`, 'success');
                        this.updateBadge();
                    } else {
                        this.showStatus(`Failed to upload ${file.name}`, 'error');
                    }
                } catch (err) {
                    this.showStatus(`Error uploading ${file.name}`, 'error');
                }
            }
        },

        async toggleFile(filename) {
            const formData = new FormData();
            formData.append('filename', filename);
            formData.append('csrfmiddlewaretoken', document.querySelector('[name=csrfmiddlewaretoken]').value);

            const response = await fetch('/memory/context/toggle/', {
                method: 'POST',
                body: formData,
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                const data = await response.json();
                this.files = data.files;
                window.contextFilesData = data.files;
            }
        },

        async deleteFile(filename) {
            if (!confirm(`Delete ${filename}?`)) return;

            const formData = new FormData();
            formData.append('filename', filename);
            formData.append('csrfmiddlewaretoken', document.querySelector('[name=csrfmiddlewaretoken]').value);

            const response = await fetch('/memory/context/delete/', {
                method: 'POST',
                body: formData,
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                const data = await response.json();
                this.files = data.files;
                window.contextFilesData = data.files;
                this.showStatus(`Deleted ${filename}`, 'success');
                this.updateBadge();
            }
        },

        showStatus(message, type) {
            this.uploadStatus = message;
            this.uploadStatusType = type;
            setTimeout(() => {
                this.uploadStatus = '';
            }, 3000);
        },

        updateBadge() {
            const badge = document.querySelector('.context-files-btn .badge');
            if (badge) {
                badge.textContent = this.files.length;
                badge.style.display = this.files.length > 0 ? 'inline' : 'none';
            }
        },

        async openEditFile(filename) {
            this.editModal.filename = filename;
            this.editModal.status = '';

            const response = await fetch(`/memory/context/content/?filename=${encodeURIComponent(filename)}`, {
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                const data = await response.json();
                this.editModal.content = data.content;
                this.editModal.show = true;
            } else {
                this.showStatus(`Failed to load ${filename}`, 'error');
            }
        },

        async saveEditFile() {
            const formData = new FormData();
            formData.append('filename', this.editModal.filename);
            formData.append('content', this.editModal.content);
            formData.append('csrfmiddlewaretoken', document.querySelector('[name=csrfmiddlewaretoken]').value);

            const response = await fetch('/memory/context/save/', {
                method: 'POST',
                body: formData,
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                this.editModal.status = 'Saved successfully';
                this.editModal.statusType = 'success';
                setTimeout(() => {
                    this.editModal.show = false;
                }, 1000);
            } else {
                this.editModal.status = 'Failed to save';
                this.editModal.statusType = 'error';
            }
        }
    };
}

// Global helper function
function openContextFilesModal() {
    if (window.contextFilesModalComponent) {
        window.contextFilesModalComponent.loadFiles();
        window.contextFilesModalComponent.showModal = true;
    }
}

// =============================================================================
// Provider Model Settings Component
// =============================================================================

function providerModelSettings() {
    return {
        // Properties with defaults (populated in init from data-* attributes)
        currentProvider: '',
        currentModel: '',
        hasExistingKey: false,
        providers: [],
        providerOpen: false,
        selectedProvider: '',
        selectedProviderName: '',
        apiKey: '',
        apiKeyModified: false,
        apiKeyValid: false,
        apiKeyError: '',
        validating: false,
        models: [],
        modelOpen: false,
        modelSearch: '',
        selectedModel: '',
        modelHighlightedIndex: 0,
        statusMessage: '',
        statusType: '',
        saving: false,

        // URLs (populated from data attributes)
        validateUrl: '',
        saveUrl: '',
        csrfToken: '',

        init() {
            const el = this.$el;
            this.currentProvider = el.dataset.provider || '';
            this.currentModel = el.dataset.model || '';
            this.hasExistingKey = el.dataset.hasExistingKey === 'true';
            this.selectedProvider = el.dataset.provider || 'openrouter';
            this.selectedProviderName = el.dataset.providerName || '';
            this.modelSearch = el.dataset.model || '';
            this.selectedModel = el.dataset.model || '';
            this.apiKeyValid = el.dataset.hasExistingKey === 'true';
            this.validateUrl = el.dataset.validateUrl || '';
            this.saveUrl = el.dataset.saveUrl || '';
            this.csrfToken = el.dataset.csrfToken || '';

            // Parse providers from JSON
            try {
                this.providers = JSON.parse(el.dataset.providers || '[]');
            } catch (e) {
                this.providers = [];
            }

            // Load models if we have an existing key
            if (this.hasExistingKey) {
                this.loadExistingModels();
            }
        },

        get currentProviderData() {
            return this.providers.find(p => p.id === this.selectedProvider);
        },

        get showModelPicker() {
            return this.apiKeyValid || this.hasExistingKey;
        },

        get filteredModels() {
            if (!this.modelSearch) return this.models;
            const s = this.modelSearch.toLowerCase();
            return this.models.filter(m =>
                m.display.toLowerCase().includes(s) || m.id.toLowerCase().includes(s)
            );
        },

        get canSave() {
            const hasValidKey = this.apiKeyValid || (this.hasExistingKey && !this.apiKeyModified);
            return hasValidKey && this.selectedModel;
        },

        selectProvider(provider) {
            this.selectedProvider = provider.id;
            this.selectedProviderName = provider.name;
            this.providerOpen = false;
            if (provider.id !== this.currentProvider) {
                this.apiKey = '';
                this.apiKeyModified = true;
                this.apiKeyValid = false;
                this.models = [];
                this.selectedModel = '';
                this.modelSearch = '';
            }
        },

        onApiKeyChange() {
            this.apiKeyModified = true;
            this.apiKeyValid = false;
            this.apiKeyError = '';
        },

        async loadExistingModels() {
            try {
                const formData = new FormData();
                formData.append('provider', this.selectedProvider);
                formData.append('use_existing', 'true');

                const response = await fetch(this.validateUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': this.csrfToken },
                    body: formData
                });
                const data = await response.json();
                if (data.valid && data.models) {
                    this.models = data.models;
                    // Update modelSearch to show display name of current model
                    const currentModel = this.models.find(m => m.id === this.selectedModel);
                    if (currentModel) {
                        this.modelSearch = currentModel.display;
                    }
                }
            } catch (e) {
                console.error('Failed to load models:', e);
                this.statusMessage = 'Failed to load models. Please try again.';
                this.statusType = 'error';
            }
        },

        async validateApiKey() {
            this.validating = true;
            this.apiKeyError = '';

            try {
                const formData = new FormData();
                formData.append('provider', this.selectedProvider);
                formData.append('api_key', this.apiKey);

                const response = await fetch(this.validateUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': this.csrfToken },
                    body: formData
                });
                const data = await response.json();

                if (data.valid) {
                    this.apiKeyValid = true;
                    this.models = data.models || [];
                    this.selectedModel = '';
                    this.modelSearch = '';
                } else {
                    this.apiKeyError = data.error || 'Invalid API key';
                }
            } catch (e) {
                this.apiKeyError = 'Validation failed. Please try again.';
            } finally {
                this.validating = false;
            }
        },

        selectModel(model) {
            this.selectedModel = model.id;
            this.modelSearch = model.display;
            this.modelOpen = false;
        },

        selectHighlightedModel() {
            if (this.filteredModels.length > 0) {
                this.selectModel(this.filteredModels[this.modelHighlightedIndex]);
            }
        },

        highlightNextModel() {
            if (this.modelHighlightedIndex < this.filteredModels.length - 1) {
                this.modelHighlightedIndex++;
                this.scrollToHighlightedModel();
            }
        },

        highlightPrevModel() {
            if (this.modelHighlightedIndex > 0) {
                this.modelHighlightedIndex--;
                this.scrollToHighlightedModel();
            }
        },

        scrollToHighlightedModel() {
            this.$nextTick(() => {
                scrollDropdownToHighlighted(this.$root, this.modelHighlightedIndex);
            });
        },

        async saveProviderModel() {
            this.saving = true;
            this.statusMessage = '';

            try {
                const formData = new FormData();
                formData.append('provider', this.selectedProvider);
                formData.append('model', this.selectedModel);

                if (this.apiKeyModified && this.apiKey) {
                    formData.append('api_key', this.apiKey);
                } else {
                    formData.append('keep_existing_key', 'true');
                }

                const response = await fetch(this.saveUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': this.csrfToken },
                    body: formData
                });
                const data = await response.json();

                if (data.success) {
                    this.statusMessage = 'Provider and model updated successfully!';
                    this.statusType = 'success';
                    this.currentProvider = data.provider;
                    this.currentModel = data.model;
                    this.hasExistingKey = true;
                    this.apiKeyModified = false;
                    setTimeout(() => { this.statusMessage = ''; }, 3000);
                } else {
                    this.statusMessage = data.error || 'Failed to save settings';
                    this.statusType = 'error';
                }
            } catch (e) {
                this.statusMessage = 'Failed to save settings. Please try again.';
                this.statusType = 'error';
            } finally {
                this.saving = false;
            }
        }
    };
}

// =============================================================================
// Home Persona Picker Component
// =============================================================================

function homePersonaPicker() {
    return {
        open: false,
        selected: '',
        selectedDisplay: '',
        highlightedIndex: 0,
        personas: [],
        personaModels: {},
        defaultModel: '',

        get filteredPersonas() {
            return this.personas;
        },

        get currentModel() {
            return this.personaModels[this.selected] || this.defaultModel;
        },

        get currentModelDisplay() {
            const model = this.currentModel;
            // Extract just the model name (after the slash)
            return model.includes('/') ? model.split('/').pop() : model;
        },

        selectPersona(p) {
            this.selected = p.id;
            this.selectedDisplay = p.display;
            this.open = false;
        },

        selectHighlighted() {
            if (this.filteredPersonas.length > 0) {
                this.selectPersona(this.filteredPersonas[this.highlightedIndex]);
            }
        },

        highlightNext() {
            if (this.highlightedIndex < this.filteredPersonas.length - 1) {
                this.highlightedIndex++;
                this.scrollToHighlighted();
            }
        },

        highlightPrev() {
            if (this.highlightedIndex > 0) {
                this.highlightedIndex--;
                this.scrollToHighlighted();
            }
        },

        scrollToHighlighted() {
            this.$nextTick(() => {
                scrollDropdownToHighlighted(this.$root, this.highlightedIndex);
            });
        },

        init() {
            const el = this.$el;

            // Parse personas from data attribute
            try {
                this.personas = JSON.parse(el.dataset.personas || '[]');
            } catch (e) {
                this.personas = [];
            }

            this.selected = el.dataset.defaultPersona || '';

            // Load data from hidden element (survives HTMX swaps)
            const dataEl = document.getElementById('home-data');
            if (dataEl) {
                try {
                    this.personaModels = JSON.parse(dataEl.dataset.personaModels || '{}');
                } catch (e) {
                    this.personaModels = {};
                }
                this.defaultModel = dataEl.dataset.defaultModel || '';
            }

            const found = this.personas.find(p => p.id === this.selected);
            if (found) this.selectedDisplay = found.display;

            // Set timezone
            setTimezoneInput();
        }
    };
}

// =============================================================================
// Persona Settings Picker Component
// =============================================================================

function personaSettingsPicker() {
    return {
        open: false,
        search: '',
        selected: '',
        highlightedIndex: 0,
        personas: [],
        settingsUrl: '',

        get filteredPersonas() {
            const found = this.personas.find(p => p.id === this.selected);
            if (found && this.search === found.display) return this.personas;
            if (!this.search) return this.personas;
            const s = this.search.toLowerCase();
            return this.personas.filter(p => p.display.toLowerCase().includes(s) || p.id.toLowerCase().includes(s));
        },

        selectPersona(p) {
            this.selected = p.id;
            this.search = p.display;
            this.open = false;
            // Trigger HTMX preview (preserve scroll position)
            const scrollContainer = document.querySelector('#main-content .overflow-y-auto');
            const scrollPos = scrollContainer ? scrollContainer.scrollTop : 0;
            htmx.ajax('GET', this.settingsUrl + '?preview=' + p.id, {target: '#main-content', swap: 'innerHTML'}).then(() => {
                if (scrollContainer) {
                    const newScrollContainer = document.querySelector('#main-content .overflow-y-auto');
                    if (newScrollContainer) newScrollContainer.scrollTop = scrollPos;
                }
            });
        },

        selectHighlighted() {
            if (this.filteredPersonas.length > 0) {
                this.selectPersona(this.filteredPersonas[this.highlightedIndex]);
            }
        },

        highlightNext() {
            if (this.highlightedIndex < this.filteredPersonas.length - 1) {
                this.highlightedIndex++;
                this.scrollToHighlighted();
            }
        },

        highlightPrev() {
            if (this.highlightedIndex > 0) {
                this.highlightedIndex--;
                this.scrollToHighlighted();
            }
        },

        scrollToHighlighted() {
            this.$nextTick(() => {
                scrollDropdownToHighlighted(this.$root, this.highlightedIndex);
            });
        },

        init() {
            const el = this.$el;

            // Parse personas from data attribute
            try {
                this.personas = JSON.parse(el.dataset.personas || '[]');
            } catch (e) {
                this.personas = [];
            }

            this.selected = el.dataset.selectedPersona || '';
            this.settingsUrl = el.dataset.settingsUrl || '/persona/';

            const found = this.personas.find(p => p.id === this.selected);
            if (found) this.search = found.display;
        }
    };
}

// =============================================================================
// Provider Picker Component (Setup Step 1)
// =============================================================================

function providerPicker() {
    return {
        open: false,
        selectedId: '',
        selectedName: '',
        selectedProvider: null,
        providers: [],

        init() {
            const el = this.$el;

            // Parse providers from data attribute
            try {
                this.providers = JSON.parse(el.dataset.providers || '[]');
            } catch (e) {
                this.providers = [];
            }

            this.selectedId = el.dataset.selectedProvider || 'openrouter';

            // Auto-select first provider if only one, or find selected
            if (this.providers.length === 1) {
                this.selectProvider(this.providers[0]);
            } else if (this.selectedId) {
                const found = this.providers.find(p => p.id === this.selectedId);
                if (found) this.selectProvider(found);
            }
        },

        selectProvider(provider) {
            this.selectedId = provider.id;
            this.selectedName = provider.name;
            this.selectedProvider = provider;
            this.open = false;
        }
    };
}

// =============================================================================
// Model Picker Component (Setup Step 2)
// =============================================================================

function modelPicker() {
    return {
        open: false,
        search: '',
        selectedId: '',
        selectedDisplay: '',
        highlightedIndex: 0,
        models: [],

        get filteredModels() {
            if (!this.search) return this.models;
            const s = this.search.toLowerCase();
            return this.models.filter(m =>
                m.display.toLowerCase().includes(s) || m.id.toLowerCase().includes(s)
            );
        },

        selectModel(model) {
            this.selectedId = model.id;
            this.selectedDisplay = model.display;
            this.search = model.display;
            this.open = false;
            this.updateButton();
        },

        selectHighlighted() {
            if (this.filteredModels.length > 0) {
                this.selectModel(this.filteredModels[this.highlightedIndex]);
            }
        },

        highlightNext() {
            if (this.highlightedIndex < this.filteredModels.length - 1) {
                this.highlightedIndex++;
                this.scrollToHighlighted();
            }
        },

        highlightPrev() {
            if (this.highlightedIndex > 0) {
                this.highlightedIndex--;
                this.scrollToHighlighted();
            }
        },

        scrollToHighlighted() {
            this.$nextTick(() => {
                scrollDropdownToHighlighted(this.$root, this.highlightedIndex);
            });
        },

        updateButton() {
            const btn = document.getElementById('submitBtn');
            if (btn) btn.disabled = !this.selectedId;
        },

        init() {
            const el = this.$el;

            // Parse models from data attribute
            try {
                this.models = JSON.parse(el.dataset.models || '[]');
            } catch (e) {
                this.models = [];
            }

            this.selectedId = el.dataset.selectedModel || '';

            // If there's a pre-selected model, set the display and search
            if (this.selectedId) {
                const found = this.models.find(m => m.id === this.selectedId);
                if (found) {
                    this.selectedDisplay = found.display;
                    this.search = found.display;
                }
            }
            this.updateButton();
        }
    };
}
