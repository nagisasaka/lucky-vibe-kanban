/* Small, local-only demonstrations. No connected agents or project data. */
(() => {
  'use strict';

  const menuButton = document.querySelector('.menu-toggle');
  const navigation = document.querySelector('#navigation');
  menuButton?.addEventListener('click', () => {
    const expanded = menuButton.getAttribute('aria-expanded') !== 'true';
    menuButton.setAttribute('aria-expanded', String(expanded));
    navigation?.classList.toggle('is-open', expanded);
  });
  navigation?.querySelectorAll('a').forEach(link => {
    link.addEventListener('click', () => {
      menuButton?.setAttribute('aria-expanded', 'false');
      navigation.classList.remove('is-open');
    });
  });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && navigation?.classList.contains('is-open')) {
      navigation.classList.remove('is-open');
      menuButton?.setAttribute('aria-expanded', 'false');
      menuButton?.focus();
    }
  });

  const demo = document.querySelector('.workspace-window');
  const phases = {
    think: {
      question: 'What if one shortcut could take me anywhere in the project?',
      answer: 'Let’s shape that idea. We can agree on the experience, then give each part a clear place to begin.',
      status: 'A clear issue is a place to begin.',
      lanes: { planned: ['palette', 'search'], building: ['settings'], review: [] }
    },
    build: {
      question: 'Let’s add a command palette. And make search feel faster.',
      answer: 'The tasks are ready. Two agents are building while we think through what comes next.',
      status: 'Three workspaces. One project in view.',
      lanes: { planned: [], building: ['palette', 'search'], review: ['settings'] }
    },
    review: {
      question: 'I like where this is going. Let’s take a look at what’s ready.',
      answer: 'Here are the changes to review. The shared issues keep the original intent close to the result.',
      status: 'Review the result. Decide what comes next.',
      lanes: { planned: [], building: [], review: ['palette', 'search', 'settings'] }
    }
  };
  document.querySelectorAll('[data-phase]').forEach(button => {
    button.addEventListener('click', () => {
      const phase = button.dataset.phase;
      const state = phases[phase];
      if (!state || !demo) return;
      demo.dataset.currentPhase = phase;
      document.querySelectorAll('[data-phase]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
      document.getElementById('demo-question').textContent = state.question;
      document.getElementById('demo-answer').textContent = state.answer;
      document.getElementById('demo-status').textContent = state.status;
      for (const [lane, cards] of Object.entries(state.lanes)) {
        const destination = demo.querySelector(`[data-lane="${lane}"]`);
        cards.forEach(id => {
          const card = demo.querySelector(`[data-card="${id}"]`);
          if (card) destination?.append(card);
        });
        const count = demo.querySelector(`[data-count="${lane}"]`);
        if (count) count.textContent = String(cards.length);
      }
    });
  });

  document.querySelectorAll('[data-time-toggle]').forEach(button => {
    button.addEventListener('click', () => {
      const state = button.dataset.timeToggle;
      const scene = document.querySelector('.time-visual');
      if (!scene) return;
      scene.dataset.time = state;
      document.querySelectorAll('[data-time-toggle]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
      document.getElementById('time-scene-title').textContent = state === 'morning' ? 'Pick up the thread.' : 'Something to wake up to.';
    });
  });
})();
