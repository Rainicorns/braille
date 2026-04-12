import { useState } from 'react'

function App() {
  const [count, setCount] = useState(0)

  return (
    <div id="app">
      <h1>Count: {count}</h1>
      <button id="increment" onClick={() => setCount(c => c + 1)}>Increment</button>
      <button id="decrement" onClick={() => setCount(c => c - 1)}>Decrement</button>
      <span id="value">{count}</span>
    </div>
  )
}

export default App
