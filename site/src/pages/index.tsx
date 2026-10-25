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
  fn show(x: T) -> str;
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
      <main id="playground">
        <BrowserOnly>
          {() => <HomepagePlayground />}
        </BrowserOnly>
      </main>
    </Layout>
  );
}
