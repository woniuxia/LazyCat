// @vitest-environment happy-dom

import { describe, expect, it } from 'vitest';
import type { JSONContent } from '@tiptap/vue-3';

import { rewriteLocalSrc } from './legacy';
import { renderRichDescription } from './render';

function toFragment(html: string): DocumentFragment {
  const template = document.createElement('template');
  template.innerHTML = html;
  return template.content;
}

function render(doc: JSONContent, rewrite: (src: string) => string = (src) => src): DocumentFragment {
  return toFragment(renderRichDescription(rewriteLocalSrc(doc, rewrite)));
}

describe('renderRichDescription', () => {
  it('renders the supported block nodes and marks', () => {
    const fragment = render({
      type: 'doc',
      content: [
        { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Roadmap' }] },
        {
          type: 'paragraph',
          content: [
            { type: 'text', marks: [{ type: 'bold' }], text: 'Alpha' },
            { type: 'hardBreak' },
            { type: 'text', marks: [{ type: 'italic' }], text: 'Beta' },
          ],
        },
        {
          type: 'bulletList',
          content: [
            {
              type: 'listItem',
              content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Ship' }] }],
            },
          ],
        },
      ],
    });

    expect(fragment.querySelector('h2')?.textContent).toBe('Roadmap');
    expect(fragment.querySelector('strong')?.textContent).toBe('Alpha');
    expect(fragment.querySelector('br')).not.toBeNull();
    expect(fragment.querySelector('em')?.textContent).toBe('Beta');
    expect(fragment.querySelector('ul > li')?.textContent).toBe('Ship');
  });

  it('preserves allowed links and removes dangerous href values', () => {
    const fragment = render({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [
            {
              type: 'text',
              text: 'Safe',
              marks: [{ type: 'link', attrs: { href: 'https://example.com/?a=1&b=2' } }],
            },
            { type: 'text', text: ' ' },
            {
              type: 'text',
              text: 'Unsafe',
              marks: [{ type: 'link', attrs: { href: 'javascript:alert(1)' } }],
            },
          ],
        },
      ],
    });

    const links = fragment.querySelectorAll('a');
    expect(links).toHaveLength(2);
    expect(links[0].getAttribute('href')).toBe('https://example.com/?a=1&b=2');
    expect(links[0].getAttribute('rel')).toBe('noopener noreferrer');
    expect(links[1].getAttribute('href')).toBe('');
  });

  it('renders rewritten local images with their persisted metadata', () => {
    const fragment = render(
      {
        type: 'doc',
        content: [
          {
            type: 'image',
            attrs: {
              src: 'attachments/preview.png',
              alt: 'Preview <1>',
              title: 'Local image',
              attId: 42,
              uploadingId: null,
            },
          },
        ],
      },
      (src) => `asset://localhost/${src}`,
    );

    const image = fragment.querySelector('img');
    expect(image?.getAttribute('src')).toBe('asset://localhost/attachments/preview.png');
    expect(image?.getAttribute('alt')).toBe('Preview <1>');
    expect(image?.getAttribute('title')).toBe('Local image');
    expect(image?.getAttribute('data-att-id')).toBe('42');
    expect(image?.hasAttribute('data-uploading-id')).toBe(false);
  });

  it('renders file references with the attributes used by viewer interactions', () => {
    const fragment = render({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [
            {
              type: 'fileRef',
              attrs: {
                attId: 9,
                src: 'attachments/spec.pdf',
                name: 'Spec <draft> & notes',
                size: 128,
                mime: 'application/pdf',
                kind: 'attachment',
                uploadingId: null,
              },
            },
          ],
        },
      ],
    });

    const fileRef = fragment.querySelector('.rte-file-ref');
    expect(fileRef?.getAttribute('data-file-ref')).toBe('');
    expect(fileRef?.getAttribute('data-att-id')).toBe('9');
    expect(fileRef?.getAttribute('data-src')).toBe('attachments/spec.pdf');
    expect(fileRef?.getAttribute('data-name')).toBe('Spec <draft> & notes');
    expect(fileRef?.getAttribute('data-size')).toBe('128');
    expect(fileRef?.getAttribute('data-mime')).toBe('application/pdf');
    expect(fileRef?.getAttribute('data-kind')).toBe('attachment');
    expect(fileRef?.textContent).toBe('📎 Spec <draft> & notes');
  });

  it('escapes text and custom node labels instead of creating injected elements', () => {
    const injection = '<img src=x onerror="alert(1)"> & text';
    const fragment = render({
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: injection }] },
        {
          type: 'paragraph',
          content: [
            {
              type: 'fileRef',
              attrs: { src: 'C:/safe.txt', name: injection, kind: 'path' },
            },
          ],
        },
      ],
    });

    expect(fragment.querySelector('img')).toBeNull();
    const paragraphs = fragment.querySelectorAll('p');
    expect(paragraphs[0].textContent).toBe(injection);
    expect(paragraphs[1].textContent).toBe(`📎 ${injection}`);
    expect(fragment.querySelector('.rte-file-ref')?.getAttribute('data-name')).toBe(injection);
  });
});
