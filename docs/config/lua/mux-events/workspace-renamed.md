# `workspace-renamed`

{{since('nightly')}}

The `workspace-renamed` event is emitted when a workspace is renamed.

The event receives the old workspace name and new workspace name as its
arguments:

```lua
local wezterm = require 'wezterm'

wezterm.on('workspace-renamed', function(old_workspace, new_workspace)
  wezterm.log_info('workspace renamed: ' .. old_workspace .. ' -> ' .. new_workspace)
end)
```
