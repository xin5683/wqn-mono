import type { Metadata } from 'next';
import { ThemeProvider } from 'next-themes';
import { Toaster } from '@/components/ui/sonner';
import { ConsentProvider } from '@/components/cookie-consent/consent-provider';
import { ConditionalAnalytics } from '@/components/cookie-consent/conditional-analytics';
import { NextIntlClientProvider } from 'next-intl';
import { getLocale, getMessages, getTranslations } from 'next-intl/server';
import { getCanonicalSiteUrl } from '@/lib/api/url';
import './globals.css';

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations('Metadata');
  const siteName = t('siteName');
  return {
    metadataBase: new URL(getCanonicalSiteUrl()),
    title: {
      default: `${siteName} – ${t('siteDescription')}`,
      // Child pages return just their local title; the template appends the
      // localised site name so `<title>` stays consistent without each page
      // hard-coding the brand suffix.
      template: `%s – ${siteName}`,
    },
    description: t('siteFullDescription'),
  };
}

export default async function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const locale = await getLocale();
  const messages = await getMessages();
  const t = await getTranslations('Common');

  // Only pass root-level messages to avoid duplicating the full bundle
  // (the [locale] layout's provider supplies the complete set)
  const rootMessages = {
    Common: (messages as Record<string, unknown>).Common,
    CookieConsent: (messages as Record<string, unknown>).CookieConsent,
  } as typeof messages;

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className="font-sans antialiased">
        <a href="#main-content" className="skip-link">
          {t('skipToMainContent')}
        </a>
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          <NextIntlClientProvider locale={locale} messages={rootMessages}>
            <ConsentProvider>
              {children}
              <ConditionalAnalytics />
            </ConsentProvider>
          </NextIntlClientProvider>
        </ThemeProvider>
        <Toaster />
        <script
          dangerouslySetInnerHTML={{
            __html: `
              // Prevent layout shift by ensuring scrollbar space is always reserved
              (function() {
                // Calculate scrollbar width
                function getScrollbarWidth() {
                  const outer = document.createElement('div');
                  outer.style.visibility = 'hidden';
                  outer.style.overflow = 'scroll';
                  outer.style.msOverflowStyle = 'scrollbar';
                  document.body.appendChild(outer);
                  
                  const inner = document.createElement('div');
                  outer.appendChild(inner);
                  
                  const scrollbarWidth = outer.offsetWidth - inner.offsetWidth;
                  outer.parentNode.removeChild(outer);
                  
                  return scrollbarWidth;
                }
                
                // Apply scrollbar compensation
                function applyScrollbarCompensation() {
                  const scrollbarWidth = getScrollbarWidth();
                  if (scrollbarWidth > 0) {
                    document.documentElement.style.setProperty('--scrollbar-width', scrollbarWidth + 'px');
                  }
                }
                
                // Run on load and resize
                if (document.readyState === 'loading') {
                  document.addEventListener('DOMContentLoaded', applyScrollbarCompensation);
                } else {
                  applyScrollbarCompensation();
                }
                
                window.addEventListener('resize', applyScrollbarCompensation);
              })();
            `,
          }}
        />
      </body>
    </html>
  );
}
