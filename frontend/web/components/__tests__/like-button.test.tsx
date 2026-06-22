// @vitest-environment jsdom

import { fireEvent, render, screen } from '@testing-library/react';
import { NextIntlClientProvider } from 'next-intl';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { LikeButton } from '@/components/like-button';

const messages = {
  Social: {
    like: 'Like',
    unlike: 'Unlike',
    loginToLike: 'Log in to like',
  },
};

function renderWithIntl(ui: ReactNode) {
  return render(
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  );
}

describe('LikeButton', () => {
  it('renders an unauthenticated disabled button with the count', () => {
    const onToggle = vi.fn();

    renderWithIntl(
      <LikeButton
        liked={false}
        count={12}
        onToggle={onToggle}
        isAuthenticated={false}
      />
    );

    const button = screen.getByRole('button') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.textContent).toContain('12');

    fireEvent.click(button);
    expect(onToggle).not.toHaveBeenCalled();
  });

  it('calls onToggle for authenticated users', () => {
    const onToggle = vi.fn();

    renderWithIntl(
      <LikeButton liked={false} count={3} onToggle={onToggle} isAuthenticated />
    );

    fireEvent.click(screen.getByRole('button'));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('disables interaction while loading', () => {
    const onToggle = vi.fn();

    renderWithIntl(
      <LikeButton liked count={4} onToggle={onToggle} isAuthenticated loading />
    );

    const button = screen.getByRole('button') as HTMLButtonElement;
    expect(button.disabled).toBe(true);

    fireEvent.click(button);
    expect(onToggle).not.toHaveBeenCalled();
  });
});
