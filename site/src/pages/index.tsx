import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import BrowserOnly from '@docusaurus/BrowserOnly';
import useBrokenLinks from '@docusaurus/useBrokenLinks';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import { useColorMode } from '@docusaurus/theme-common';
import { Playground } from "@boltlang/playground";
import "@boltlang/playground/style.css";
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

const sample = `
// Edit this code to see it highlighted by tree-sitter and checked by the
// Bolt language server. Try a mistake, like \`let n: i32 = "one";\`.

// \`print\`, \`Option\` and the rest of the prelude of \`std\` need no import.

/// A shape, with its size.
pub enum Shape {
  Circle(f64),
  Rect { w: f64, h: f64 },
}

class Show[T] {
  fn show(x: T) -> String;
}

impl Show[Shape] {
  fn show(s) = match s {
    Shape.Circle(_) => "circle",
    Shape.Rect { .. } => "rectangle",
  };
}

const PI: f64 = 3.14159;

fn area(s: Shape) -> f64 = match s {
  Shape.Circle(r) => PI * r * r,
  Shape.Rect { w, h } => w * h,
};

struct Vec2 { x: f64, y: f64 }

fn (a: Vec2) + (b: Vec2) -> Vec2 = Vec2 { x: a.x + b.x, y: a.y + b.y };

let shapes = [Shape.Circle(1.0), Shape.Rect { w: 2.0, h: 3.0 }];
for s in shapes {
  if area(s) > 1.0 {
    print(s.show());
  }
}

macro_rules! twice {
  ($e:expr) => { ($e, $e) };
}
`;

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link
            className="button button--secondary button--lg"
            style={{ margin: '1rem' }}
            to="#playground">
            To the playground
          </Link>
          <Link
            className="button button--secondary button--lg"
            style={{ margin: '1rem' }}
            to="/docs/tutorial">
            Learn how to write Bolt
          </Link>
        </div>
      </div>
    </header>
  );
}

type Feature = {
  title: string;
  lead: ReactNode;
  points: ReactNode[];
};

const features: Feature[] = [
  {
    title: 'Secure by design',
    lead: <>Whole classes of bugs can't happen in Bolt, because the type checker rules them out before the program runs.</>,
    points: [
      <>There's no <code>null</code>. A value that may be missing is an <code>Option</code>, and the type checker makes you handle <code>None</code>.</>,
      <>Whether a value can change is part of its type. An <code>imm</code> array or struct can never change, not even through another name for it, so data you hand out stays as it was.</>,
      <>Programs compile to WebAssembly, which runs in a sandbox: compiled code can only reach the outside world through the few functions the host gives it, and every array access is checked.</>,
    ],
  },
  {
    title: 'Performant',
    lead: <>Bolt knows every type before the program runs, so the program doesn't have to find them out while it's running.</>,
    points: [
      <>Programs compile ahead of time to WebAssembly, which browsers and runtimes like Wasmtime run at close to native speed.</>,
      <>Static types mean no type checks or boxing at run time: an <code>i32</code> is a machine integer, and a call is a direct call.</>,
      <>Arrays and structs are passed by reference, never copied behind your back, and their mutability in the type tells the compiler what can't change.</>,
    ],
  },
  {
    title: 'Productive',
    lead: <>Bolt infers the types you leave out, and tells you what's wrong while you type.</>,
    points: [
      <>Write <code>fn first(c) = c[0];</code> and Bolt works out that it's generic: it takes an array, a <code>Vec</code>, or anything else that can be indexed.</>,
      <>A language server reports mistakes in your editor, and in the playground above, with errors that point at the cause as well as the symptom.</>,
      <>Run a program right away with <code>bolt run</code>, then compile the same code with <code>bolt build</code>. Enums with data, pattern matching, classes and macros keep the code short.</>,
    ],
  },
];

function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {features.map(({title, lead, points}) => (
            <div key={title} className={clsx('col col--4', styles.feature)}>
              <Heading as="h2">{title}</Heading>
              <p className={styles.lead}>{lead}</p>
              <ul>
                {points.map((point, i) => <li key={i}>{point}</li>)}
              </ul>
            </div>
          ))}
        </div>
        <div className={styles.buttons}>
          <Link className="button button--primary button--lg" to="/docs/tutorial">
            Start the tutorial
          </Link>
        </div>
      </div>
    </section>
  );
}

// The playground, in the colors of the site, which has a toggle of its own.
function HomepagePlayground(): ReactNode {
  const {colorMode} = useColorMode();
  return <Playground theme={colorMode} style={{height: '100vh'}} defaultValue={sample} />;
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  // Docusaurus only knows the anchors that components report, like those of
  // headings, so the check of the link to `#playground` needs this one.
  useBrokenLinks().collectAnchor('playground');
  return (
    <Layout
      title={siteConfig.title}
      description="Bolt is a language for quickly writing highly performant and safe (web) applications.">
      <HomepageHeader />
      <main>
        <div id="playground">
          <BrowserOnly>
            {() => <HomepagePlayground />}
          </BrowserOnly>
        </div>
        <HomepageFeatures />
      </main>
    </Layout>
  );
}
