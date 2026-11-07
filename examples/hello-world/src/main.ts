import './style.css'
import { run } from './hello.bolt'

// What the Bolt program prints, a line at a time.
const output = document.querySelector<HTMLDivElement>('#app')!
run({
  stdout: (line) => {
    const p = document.createElement('p')
    p.textContent = line
    output.append(p)
  },
  stderr: (line) => console.error(line),
})
