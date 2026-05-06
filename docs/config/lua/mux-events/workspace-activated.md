# `workspace-activated`

{{since('nightly')}}

The `workspace-activated` event is emitted when the active workspace changes
for the current GUI client.

The event receives the activated workspace name as its argument:

```lua
local wezterm = require 'wezterm'

wezterm.on('workspace-activated', function(workspace)
  wezterm.log_info('workspace activated: ' .. workspace)
end)
```
