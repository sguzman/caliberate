# GUI News Roadmap

## News Sources
- [x] Add news source list UI
- [x] Add news source enable/disable toggles
- [x] Add news source scheduling editor
- [x] Add news source search/filter box
- [x] Add custom recipe import UI

## Downloads
- [x] Add fetched news list and status
- [x] Add download history and logs view
- [x] Add news download retry action
- [x] Add per-source download status badges

## Reader Integration
- [x] Add open news in reader action
- [x] Add news collection grouping in library view
- [x] Add news auto-delete retention settings UI

## Parity Gap Backlog
- [ ] Replace generated placeholder digest writer with real article fetch pipeline per source
- [ ] Implement recipe execution engine compatible with imported recipe definitions
- [ ] Build real news issue packaging (HTML/EPUB) instead of plain `.txt` placeholder files
- [ ] Add per-source fetch deduplication (source + publication date + article URL hash)
- [ ] Persist per-source fetch cursor/checkpoint to avoid re-import loops after restart
- [ ] Add article-level preview pane and link list in news manager dialog
