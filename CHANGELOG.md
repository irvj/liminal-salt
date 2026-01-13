# Changelog

## [0.5.8] - 2026-01-12

### Changes
- update readme for context upload features
- updates to claude file for persona context feature
- add persona specific context file upload

## [0.5.7] - 2026-01-12

### Changes
- add draft feature for new chat page
- add draft feature for chat threads
- add better error handling to chat

## [0.5.6] - 2026-01-12

### Changes
- fix button text, update screenshots

## [0.5.5] - 2026-01-12

### Changes
- add chat screenshot
- fix dark mode chat user text

## [0.5.4] - 2026-01-12

### Changes
- add screenshots, update readme

## [0.5.3] - 2026-01-12

### Changes
- update readme
- update claude file with new liminal salt default
- add liminal salt theme, make it default, various theme style tweaks

## [0.5.2] - 2026-01-12

### Changes
- add more themes

## [0.5.1] - 2026-01-12

### Changes
- update claude file for new theme picker feature
- add theme picker feature, add themes

## [0.5.0] - 2026-01-12

### Changes
- Merge branch 'javascript-refactor'
- update claude file for new javascript architecture
- separate inline javascript to static component and utils files for better separation of concerns

## [0.4.0] - 2026-01-12

### Changes
- update claude file
- fix persona page load lag
- fix issues with data parsing on window swaps

## [0.3.0] - 2026-01-12

### Changes
- rename personalities to personas, create standalone persona settings page
- fix openrouter metadata submission
- further tweaks to chat ux

## [0.2.9] - 2026-01-11

### Changes
- various qol updates to chat window
- pass liminal salt app name and site to openrouter

## [0.2.8] - 2026-01-11

### Changes
- add custom model setting for personalities

## [0.2.7] - 2026-01-11

### Changes
- update claude file
- prevent default assistant personality from being deleted
- change new chat svg to circle plus
- add chevron down to personality picker in settings
- add x svg

## [0.2.6] - 2026-01-11

### Changes
- move personality picker on new chat page
- convert all emojis to svgs
- add sidebar footer, migrate existing buttons
- fix style and functionality issues when all chat threads deleted
- add ability to edit chat thread title
- add pinned session feature

## [0.2.5] - 2026-01-11

### Changes
- standardize browser title
- fix edit personality data issue
- update styling for personality picker
- settings page style fixes
- fix model picker dropdowns for arrow navigation
- add provider and model picker in settings

## [0.2.4] - 2026-01-11

### Changes
- setup: add select for future provider picker, add openrouter api validation, update styling
- convert settings page personality selector to alpine implementation
- add hover states to settings modal buttons
- add assistant name to selector
- better mobile responsiveness for sidenav

## [0.2.3] - 2026-01-11

### Changes
- initial commit of new chat page

## [0.2.2] - 2026-01-11

### Changes
- chat button styling, delete modal button style fixes
- add sidebar collapse
- fix various light mode styles
- typewriter effect on assistant response

## [0.2.1] - 2026-01-10

### Changes
- add launcher script, use waitress and whitenoise for server, fix timezone issue, update instructions

## [0.2.0] - 2026-01-10

### Changes
- claude file update
- remove version warning
- Merge branch 'css-refactor'
- minify output
- changes to nord light mode
- add minify command to watcher
- add tailwind typography plugin, fix markdown display containers
- fix chat list display issue
- add version bump script
- initial commit of tailwind conversion
- Merge branch 'fix-sidenav-delete-reload'
- refactor for reactivity of sidebar on chat delete
- Merge branch 'fix-default-personality-logic'
- add better handling of default personality with new chats
- more reinforcement of time logic
- add version notice to goals
- reenforce time to assistant
- Merge branch 'chat-timestamps'
- initial commit of new timestamp and time parsing feature
- add goals and roadmap section
- remove personality section from readme
- Merge branch 'user-file-upload'
- update gitignore
- initial commit of user context file upload tool
- fix memory input styling
- style and functionality tweaks to chat form field
- fix text alignment
- add delay on assistant messages
- chat styling updates
- fix sidenav item select styling issue
- fix redirect bug when wiping and regen user memory
- Merge branch 'settings-page-restyle'
- add delete personality feature
- add new personality file functionality
- add edit personality file feature, remove orphaed pages
- restyle settings page
- Merge branch 'riddler-assistant'
- add riddler personality
- bump version
- Merge branch 'user-memory-updates'
- fix title, remove emojis, styling
- add memory update feature, styling tweaks
- update user memory container styling
- add version number
- add readme and license
- Merge branch 'migrate-to-django'
- update claude file for django migration
- refactor folder structure
- fix settings functionality
- fix user memory feature
- initial commit of django migration
- add pricing to model select
- update name of app
- add config creator
- initial commit of liminal salt app
