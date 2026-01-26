# DevTools Protocol Reference

Complete documentation for Graviton's Chrome DevTools Protocol (CDP) compatible interface.

## Protocol Format

### Request
```json
{"id": 1, "method": "DOM.getDocument", "params": {"depth": 2}}
```

### Success Response
```json
{"id": 1, "result": {"root": {...}}}
```

### Error Response
```json
{"id": 1, "error": {"code": -32602, "message": "Invalid params"}}
```

## Command Reference

### DOM Domain

| Method | Description | Parameters |
|--------|-------------|------------|
| `DOM.enable` | Enable DOM domain | - |
| `DOM.disable` | Disable DOM domain | - |
| `DOM.getDocument` | Get document tree | `depth` (int, optional) |
| `DOM.querySelector` | Find first matching element | `nodeId`, `selector` |
| `DOM.querySelectorAll` | Find all matching elements | `nodeId`, `selector` |
| `DOM.getBoxModel` | Get element geometry | `nodeId` |
| `DOM.getAttributes` | Get element attributes | `nodeId` |
| `DOM.getOuterHTML` | Get element HTML | `nodeId` |
| `DOM.describeNode` | Describe a node | `nodeId` or `backendNodeId`, `depth` |

### CSS Domain

| Method | Description | Parameters |
|--------|-------------|------------|
| `CSS.enable` | Enable CSS domain | - |
| `CSS.disable` | Disable CSS domain | - |
| `CSS.getComputedStyleForNode` | Get computed styles | `nodeId` |
| `CSS.getMatchedStylesForNode` | Get matched CSS rules | `nodeId` |

### Page Domain

| Method | Description | Parameters |
|--------|-------------|------------|
| `Page.enable` | Enable Page domain | - |
| `Page.disable` | Disable Page domain | - |
| `Page.navigate` | Navigate to URL | `url` |
| `Page.reload` | Reload current page | - |
| `Page.captureScreenshot` | Capture screenshot | `format` (png/jpeg), `quality` |
| `Page.getLayoutMetrics` | Get viewport/content sizes | - |

### Input Domain

| Method | Description | Parameters |
|--------|-------------|------------|
| `Input.enable` | Enable Input domain | - |
| `Input.disable` | Disable Input domain | - |
| `Input.dispatchMouseEvent` | Send mouse event | `type`, `x`, `y`, `button`, `clickCount` |
| `Input.dispatchKeyEvent` | Send keyboard event | `type`, `key`, `code`, `text` |

### Runtime Domain

| Method | Description | Parameters |
|--------|-------------|------------|
| `Runtime.enable` | Enable Runtime domain | - |
| `Runtime.disable` | Disable Runtime domain | - |
| `Runtime.evaluate` | Evaluate expression | `expression` |

### Overlay Domain

| Method | Description | Parameters |
|--------|-------------|------------|
| `Overlay.enable` | Enable Overlay domain | - |
| `Overlay.disable` | Disable Overlay domain, clears highlight | - |
| `Overlay.highlightNode` | Highlight an element | `nodeId` or `backendNodeId` |
| `Overlay.hideHighlight` | Clear element highlight | - |

**Note:** When an element is highlighted via `Overlay.highlightNode`, subsequent `Page.captureScreenshot` calls will include the DevTools-style overlay visualization showing:
- **Orange** - margin box
- **Green** - padding box
- **Blue** - content box

## Data Formats

### Node Types

| Value | Type |
|-------|------|
| 1 | Element |
| 3 | Text |
| 8 | Comment |
| 9 | Document |
| 10 | DocumentType |

### CDP Node Format

```json
{
  "nodeId": 5,
  "backendNodeId": 5,
  "nodeType": 1,
  "nodeName": "DIV",
  "localName": "div",
  "nodeValue": "",
  "childNodeCount": 3,
  "children": [...],
  "attributes": ["class", "container", "id", "main"]
}
```

Note: `attributes` is a flat array `[name1, value1, name2, value2, ...]` per CDP spec.

### Box Model Format

```json
{
  "content": [x1, y1, x2, y2, x3, y3, x4, y4],
  "padding": [...],
  "border": [...],
  "margin": [...],
  "width": 200,
  "height": 100
}
```

Each array contains 8 values representing a quad (4 corners, clockwise from top-left).

## Error Codes

| Code | Meaning |
|------|---------|
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |

## Limitations

### Supported Selectors

- Tag name: `div`, `span`, `button`
- Class: `.my-class`
- ID: `#my-id`

Complex selectors (combinators, attribute selectors) are not yet supported.

### Runtime.evaluate

Supports limited expressions:
- `document.title`, `document.URL`, `document.body`
- `document.getElementById("id")`
- Literals: numbers, strings, booleans, null

Full JavaScript execution is not supported.

## Python Example

```python
import subprocess
import json

class GravitonAgent:
    def __init__(self, html_file, width=1366, height=768):
        self.process = subprocess.Popen(
            ['cargo', 'run', '--bin', 'devtools_agent', '--',
             html_file, str(width), str(height)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )
        self._id = 0

    def send(self, method, params=None):
        self._id += 1
        request = {"id": self._id, "method": method, "params": params or {}}
        self.process.stdin.write(json.dumps(request) + '\n')
        self.process.stdin.flush()
        response = self.process.stdout.readline()
        return json.loads(response)

    def close(self):
        self.process.terminate()

# Usage
agent = GravitonAgent('test.html')
doc = agent.send('DOM.getDocument', {'depth': 3})
result = agent.send('DOM.querySelector', {'nodeId': 1, 'selector': '.my-class'})
styles = agent.send('CSS.getComputedStyleForNode', {'nodeId': result['result']['nodeId']})
screenshot = agent.send('Page.captureScreenshot', {'format': 'png'})
agent.close()
```

## Common Workflows

### Inspect Page Structure
```python
doc = agent.send('DOM.getDocument', {'depth': -1})  # -1 = unlimited depth
```

### Get Element Position & Size
```python
result = agent.send('DOM.querySelector', {'nodeId': 1, 'selector': '#target'})
box = agent.send('DOM.getBoxModel', {'nodeId': result['result']['nodeId']})
# box['result']['model'] has content, padding, border, margin quads
```

### Get Computed Styles
```python
styles = agent.send('CSS.getComputedStyleForNode', {'nodeId': node_id})
style_map = {p['name']: p['value'] for p in styles['result']['computedStyle']}
```

### Mouse Interaction
```python
agent.send('Input.dispatchMouseEvent', {'type': 'mousePressed', 'x': 100, 'y': 200, 'button': 'left'})
agent.send('Input.dispatchMouseEvent', {'type': 'mouseReleased', 'x': 100, 'y': 200, 'button': 'left'})
```

### Highlight Element & Capture Screenshot
```python
# Find element
result = agent.send('DOM.querySelector', {'nodeId': 1, 'selector': '.my-element'})
node_id = result['result']['nodeId']

# Highlight it
agent.send('Overlay.highlightNode', {'nodeId': node_id})

# Screenshot now includes the highlight overlay (margin=orange, padding=green, content=blue)
screenshot = agent.send('Page.captureScreenshot', {'format': 'png'})

# Clear highlight when done
agent.send('Overlay.hideHighlight')
```
