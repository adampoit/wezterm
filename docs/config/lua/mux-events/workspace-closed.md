# `workspace-closed`

{{since('nightly')}}

The `workspace-closed` event is emitted when the mux observes that the last
pane in a workspace has closed.

The event receives the workspace name as its argument:

```lua
local wezterm = require 'wezterm'

wezterm.on('workspace-closed', function(workspace)
  wezterm.log_info('workspace closed: ' .. workspace)
end)
```
